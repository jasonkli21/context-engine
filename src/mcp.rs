//! MCP 2026-07-28 stdio binding through the maintained, pinned official SDK.
use crate::{
    adapter::{self, Request, Worker},
    application::Application,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::{NotificationContext, RequestContext, Service},
    transport::Transport,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    future::Future,
    io,
    sync::{Arc, Mutex},
};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone)]
struct Mcp {
    worker: Worker,
}
pub const TOOLS: [&str; 5] = [
    "search_codebase",
    "find_symbol",
    "get_dependency_neighborhood",
    "get_change_impact",
    "build_task_context",
];
fn tool<T: schemars::JsonSchema>(name: &'static str, description: &'static str) -> Tool {
    let schema = schemars::schema_for!(T);
    let mut input_schema = schema.as_object().unwrap().clone();
    if name == TOOLS[2] {
        input_schema.insert(
            "oneOf".into(),
            serde_json::json!([
                {"required":["symbol"],"not":{"required":["file"]}},
                {"required":["file"],"not":{"required":["symbol"]}}
            ]),
        );
    }
    Tool::new(name, description, Arc::new(input_schema)).with_annotations(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .open_world(false),
    )
}
fn tools() -> Vec<Tool> {
    vec![
        tool::<adapter::Search>(
            TOOLS[0],
            "Search current file paths and symbols; syntax evidence, index-local IDs.",
        ),
        tool::<adapter::Symbol>(
            TOOLS[1],
            "Exact current symbol lookup; returns all ambiguous matches.",
        ),
        tool::<adapter::Neighborhood>(
            TOOLS[2],
            "Bounded current syntax neighborhood, depth 0..8; exactly one symbol or relative file.",
        ),
        tool::<adapter::Impact>(
            TOOLS[3],
            "Immutable committed BASE..HEAD change impact, depth 0..8.",
        ),
        tool::<adapter::Context>(
            TOOLS[4],
            "Full-file source context, file-ranker-v1; text fields have a 4096 UTF-8 byte runtime cap; budget 1..1048576 UTF-8 bytes. Conventional credential files are omitted from content. Exposes other source contents.",
        ),
    ]
}
impl ServerHandler for Mcp {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![ProtocolVersion::V_2026_07_28])
    }
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("context-engine", env!("CARGO_PKG_VERSION")),
        )
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.and_then(|p| p.cursor).is_some() {
            return Err(ErrorData::invalid_params(
                "pagination cursor unsupported",
                None,
            ));
        }
        Ok(ListToolsResult {
            tools: tools(),
            ..Default::default()
        })
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|t| t.name == name)
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !TOOLS.contains(&r.name.as_ref()) {
            return Err(ErrorData::method_not_found::<CallToolRequestMethod>());
        }
        let bytes = serde_json::to_vec(&r.arguments.unwrap_or_default())
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
        let request = match Request::parse_detailed(&r.name, &bytes) {
            Ok(request) => request,
            Err(adapter::ParseError::Invalid) => {
                return Err(ErrorData::invalid_params("invalid arguments", None));
            }
            Err(adapter::ParseError::Semantic) => {
                return Ok(CallToolResult::structured_error(serde_json::json!({
                    "api_version": 1,
                    "error": {"code": "invalid_arguments", "message": "arguments do not satisfy operation constraints"}
                })).into());
            }
        };
        let result = match self
            .worker
            .run(request, |reply| {
                let result = CallToolResult::structured(
                    serde_json::to_value(reply).map_err(|_| adapter::Error::Internal)?,
                );
                // Text plus structured content and outer protocol metadata/ID reserve.
                adapter::check_size(&result, adapter::MAX_RESPONSE - adapter::MAX_REQUEST)?;
                Ok(result)
            })
            .await
        {
            Ok(result) => result,
            Err(e) => CallToolResult::structured_error(
                serde_json::json!({"api_version":1,"error":{"code":e,"message":e.to_string()}}),
            ),
        };
        Ok(result.into())
    }
}
pub async fn run(app: Application) -> crate::application::Result<()> {
    let worker = Worker::new(app);
    let transport = StdioTransport::new(tokio::io::stdin(), tokio::io::stdout());
    // Modern-only service: skip the SDK's dual-era bootstrap, which otherwise
    // rejects notifications/EOF before the first non-discover request.
    let service = rmcp::service::serve_directly(
        Modern(Mcp {
            worker: worker.clone(),
        }),
        transport,
        None,
    );
    let result = service.waiting().await;
    worker.drain().await;
    result?;
    Ok(())
}

/// Keep SDK dispatch while correcting its deliberate malformed-JSON drop
/// behavior to JSON-RPC's required parse-error response.
struct StdioTransport<R, W> {
    reader: tokio::io::BufReader<R>,
    writer: Arc<tokio::sync::Mutex<W>>,
    line: Vec<u8>,
    discarding: bool,
    outstanding: Arc<Mutex<OutstandingRequests>>,
}
#[derive(Default)]
struct OutstandingRequests {
    by_wire_id: HashMap<RequestId, PendingRequest>,
    by_internal_id: HashMap<RequestId, RequestId>,
    next_internal_id: u64,
}
struct PendingRequest {
    internal_id: RequestId,
    response_sending: bool,
}
impl OutstandingRequests {
    fn admit(&mut self, wire_id: RequestId) -> Option<RequestId> {
        if self.by_wire_id.contains_key(&wire_id) || self.by_wire_id.len() >= 16 {
            return None;
        }
        // Incoming IDs are always translated, so this private namespace cannot
        // collide with any client-supplied numeric or string ID.
        let internal_id = loop {
            self.next_internal_id = self.next_internal_id.wrapping_add(1);
            let candidate =
                RequestId::String(Arc::from(format!("ctx-internal-{}", self.next_internal_id)));
            if !self.by_internal_id.contains_key(&candidate) {
                break candidate;
            }
        };
        self.by_internal_id
            .insert(internal_id.clone(), wire_id.clone());
        self.by_wire_id.insert(
            wire_id,
            PendingRequest {
                internal_id: internal_id.clone(),
                response_sending: false,
            },
        );
        Some(internal_id)
    }
    fn internal_for_wire(&self, wire_id: &RequestId) -> Option<RequestId> {
        self.by_wire_id
            .get(wire_id)
            .map(|pending| pending.internal_id.clone())
    }
    fn release_wire(&mut self, wire_id: &RequestId) {
        if let Some(pending) = self.by_wire_id.remove(wire_id) {
            self.by_internal_id.remove(&pending.internal_id);
        }
    }
    fn release_internal(&mut self, internal_id: &RequestId) {
        if let Some(wire_id) = self.by_internal_id.get(internal_id).cloned()
            && self
                .by_wire_id
                .get(&wire_id)
                .is_some_and(|pending| pending.internal_id == *internal_id)
        {
            self.release_wire(&wire_id);
        }
    }
}
impl<R: tokio::io::AsyncRead, W> StdioTransport<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader: tokio::io::BufReader::new(reader),
            writer: Arc::new(tokio::sync::Mutex::new(writer)),
            line: Vec::new(),
            discarding: false,
            outstanding: Arc::new(Mutex::new(OutstandingRequests::default())),
        }
    }
}
async fn write_message<W: AsyncWrite + Unpin>(
    writer: &Arc<tokio::sync::Mutex<W>>,
    item: &ServerJsonRpcMessage,
) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(item).map_err(io::Error::other)?;
    bytes.push(b'\n');
    let mut writer = writer.lock().await;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        writer.write_all(&bytes).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
    Ok(())
}
impl<R, W> StdioTransport<R, W>
where
    R: tokio::io::AsyncRead + Unpin,
{
    async fn next_line(&mut self) -> io::Result<Option<Result<Vec<u8>, ()>>> {
        loop {
            let (consumed, newline, over_limit) = {
                let available = self.reader.fill_buf().await?;
                if available.is_empty() {
                    return Ok(None);
                }
                let newline = available.iter().position(|byte| *byte == b'\n');
                let take = newline.map_or(available.len(), |index| index + 1);
                let over_limit =
                    self.discarding || self.line.len().saturating_add(take) > adapter::MAX_REQUEST;
                if over_limit {
                    self.line.clear();
                } else {
                    self.line.extend_from_slice(&available[..take]);
                }
                (take, newline.is_some(), over_limit)
            };
            self.reader.consume(consumed);
            if over_limit && !newline {
                self.discarding = true;
                continue;
            }
            if newline {
                self.discarding = false;
                if over_limit {
                    self.line.clear();
                    return Ok(Some(Err(())));
                }
                return Ok(Some(Ok(std::mem::take(&mut self.line))));
            }
        }
    }
}
impl<R, W> Transport<RoleServer> for StdioTransport<R, W>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
    W: tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    type Error = io::Error;
    fn send(
        &mut self,
        item: ServerJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let writer = self.writer.clone();
        let pending = self.outstanding.clone();
        let internal_id = match &item {
            ServerJsonRpcMessage::Response(response) => Some(response.id.clone()),
            ServerJsonRpcMessage::Error(error) => error.id.clone(),
            _ => None,
        };
        let mut item = item;
        let wire_id = internal_id.as_ref().and_then(|id| {
            let mut pending = pending.lock().unwrap();
            let wire_id = pending.by_internal_id.get(id)?.clone();
            let slot = pending.by_wire_id.get_mut(&wire_id)?;
            if slot.internal_id != *id {
                return None;
            }
            slot.response_sending = true;
            Some(wire_id)
        });
        let discard_stale_response = internal_id.is_some() && wire_id.is_none();
        if let Some(wire_id) = &wire_id {
            match &mut item {
                ServerJsonRpcMessage::Response(response) => response.id = wire_id.clone(),
                ServerJsonRpcMessage::Error(error) => error.id = Some(wire_id.clone()),
                _ => {}
            }
        }
        async move {
            if discard_stale_response {
                return Ok(());
            }
            write_message(&writer, &item).await?;
            if let Some(id) = internal_id {
                pending.lock().unwrap().release_internal(&id);
            }
            Ok(())
        }
    }
    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        loop {
            let line = match self.next_line().await {
                Ok(Some(Ok(line))) => line,
                Ok(Some(Err(()))) => continue,
                Ok(None) | Err(_) => return None,
            };
            let line = line.strip_suffix(b"\n").unwrap_or(&line);
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            match serde_json::from_slice(line) {
                Ok(mut message) => match &mut message {
                    ClientJsonRpcMessage::Request(request) => {
                        let rejection = {
                            let mut pending = self.outstanding.lock().unwrap();
                            if pending.by_wire_id.contains_key(&request.id) {
                                // Null ID avoids correlating this rejection with the
                                // admitted original or releasing its response slot.
                                Some((None, "Duplicate outstanding request ID"))
                            } else if pending.by_wire_id.len() >= 16 {
                                Some((
                                    Some(request.id.clone()),
                                    "Outstanding request limit reached",
                                ))
                            } else {
                                let wire_id = request.id.clone();
                                request.id = pending
                                    .admit(wire_id)
                                    .expect("capacity and duplicate checks matched admission");
                                None
                            }
                        };
                        if let Some((id, reason)) = rejection {
                            let error =
                                JsonRpcError::new(id, ErrorData::invalid_request(reason, None));
                            if write_message(&self.writer, &ServerJsonRpcMessage::Error(error))
                                .await
                                .is_err()
                            {
                                return None;
                            }
                            continue;
                        }
                        return Some(message);
                    }
                    ClientJsonRpcMessage::Notification(notification) => {
                        if let ClientNotification::CancelledNotification(cancelled) =
                            &mut notification.notification
                            && let Some(wire_id) = &cancelled.params.request_id
                        {
                            let mut pending = self.outstanding.lock().unwrap();
                            if let Some(internal_id) = pending.internal_for_wire(wire_id) {
                                // If send has started, the response owns this ID
                                // until the write finishes. Otherwise remap the
                                // cancellation to the SDK's private request ID and
                                // release the wire ID; the SDK drops any later
                                // completion for the cancelled private ID.
                                if !pending
                                    .by_wire_id
                                    .get(wire_id)
                                    .is_some_and(|slot| slot.response_sending)
                                {
                                    pending.release_wire(wire_id);
                                }
                                cancelled.params.request_id = Some(internal_id);
                            }
                        }
                        return Some(message);
                    }
                    _ => return Some(message),
                },
                Err(error) => {
                    let data = match error.classify() {
                        serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                            ErrorData::parse_error("Invalid JSON", None)
                        }
                        serde_json::error::Category::Data | serde_json::error::Category::Io => {
                            ErrorData::invalid_request("Invalid request", None)
                        }
                    };
                    let message = ServerJsonRpcMessage::Error(JsonRpcError::new(None, data));
                    if write_message(&self.writer, &message).await.is_err() {
                        return None;
                    }
                }
            }
        }
    }
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.writer.lock().await.flush().await
    }
}

struct Modern(Mcp);
impl Service<RoleServer> for Modern {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerResult, ErrorData> {
        let version = ProtocolVersion::V_2026_07_28;
        if let Some(requested) = context.meta.protocol_version()
            && requested != version
        {
            return Err(ErrorData::unsupported_protocol_version(
                requested,
                &[version],
            ));
        }
        if !context.meta.missing_required_keys(&version).is_empty() {
            return Err(ErrorData::invalid_params(
                "required MCP 2026-07-28 request metadata missing or malformed",
                None,
            ));
        }
        if matches!(request, ClientRequest::InitializeRequest(_)) {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "initialize is not supported",
                None,
            ));
        }
        self.0.handle_request(request, context).await
    }
    async fn handle_notification(
        &self,
        notification: ClientNotification,
        context: NotificationContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.0.handle_notification(notification, context).await
    }
    fn get_info(&self) -> ServerConfig {
        ServerHandler::get_info(&self.0)
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        ServerHandler::supported_protocol_versions(&self.0)
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use serde_json::{Value, json};
    // Withhold all sends to make the outstanding count deterministic without
    // depending on worker/Git latency or SDK response scheduling.
    #[tokio::test]
    async fn duplicate_and_capacity_errors_preserve_admitted_work_and_transport() {
        let (mut client, input) = tokio::io::duplex(32 * 1024);
        let (output, wire) = tokio::io::duplex(32 * 1024);
        let mut wire = tokio::io::BufReader::new(wire);
        let mut transport = StdioTransport::new(input, output);
        async fn put(client: &mut tokio::io::DuplexStream, value: Value) {
            client
                .write_all(format!("{value}\n").as_bytes())
                .await
                .unwrap();
        }
        let mut internal_ids = Vec::new();
        for id in 0..15 {
            put(
                &mut client,
                json!({"jsonrpc":"2.0", "id":id, "method":"ping"}),
            )
            .await;
        }
        put(
            &mut client,
            json!({"jsonrpc":"2.0", "id":"fifteen", "method":"ping"}),
        )
        .await;
        // All requests arrive before any response, including duplicate-at-cap.
        for id in [0, 16, 17] {
            put(
                &mut client,
                json!({"jsonrpc":"2.0", "id":id, "method":"ping"}),
            )
            .await;
        }
        put(
            &mut client,
            json!({"jsonrpc":"2.0", "method":"notifications/cancelled", "params":{"requestId":999}}),
        )
        .await;
        put(
            &mut client,
            json!({"jsonrpc":"2.0", "method":"notifications/cancelled", "params":{"requestId":"fifteen"}}),
        )
        .await;
        for _ in 0..16 {
            let Some(ClientJsonRpcMessage::Request(request)) = transport.receive().await else {
                panic!("admitted request expected");
            };
            internal_ids.push(request.id);
        }
        assert_eq!(transport.outstanding.lock().unwrap().by_wire_id.len(), 16);
        assert!(matches!(
            transport.receive().await,
            Some(ClientJsonRpcMessage::Notification(_))
        ));
        assert_eq!(transport.outstanding.lock().unwrap().by_wire_id.len(), 16);
        assert!(matches!(
            transport.receive().await,
            Some(ClientJsonRpcMessage::Notification(_))
        ));
        for id in [Value::Null, json!(16), json!(17)] {
            let mut line = String::new();
            wire.read_line(&mut line).await.unwrap();
            let error: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(error["id"], id);
            assert_eq!(error["error"]["code"], -32600);
        }
        assert_eq!(transport.outstanding.lock().unwrap().by_wire_id.len(), 15);
        // An actual admitted response still flushes and releases exactly its slot.
        transport
            .send(ServerJsonRpcMessage::Error(JsonRpcError::new(
                Some(internal_ids[0].clone()),
                ErrorData::invalid_params("fixture", None),
            )))
            .await
            .unwrap();
        let mut line = String::new();
        wire.read_line(&mut line).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 0);
        assert_eq!(transport.outstanding.lock().unwrap().by_wire_id.len(), 14);
        put(
            &mut client,
            json!({"jsonrpc":"2.0", "id":18, "method":"ping"}),
        )
        .await;
        assert!(matches!(
            transport.receive().await,
            Some(ClientJsonRpcMessage::Request(_))
        ));
        drop(client);
        assert!(transport.receive().await.is_none());
    }

    #[derive(Clone)]
    struct DelayedService {
        started: tokio::sync::mpsc::UnboundedSender<(
            RequestId,
            tokio::sync::oneshot::Sender<Result<ServerResult, ErrorData>>,
        )>,
    }
    impl Service<RoleServer> for DelayedService {
        async fn handle_request(
            &self,
            _: ClientRequest,
            context: RequestContext<RoleServer>,
        ) -> Result<ServerResult, ErrorData> {
            let (complete, response) = tokio::sync::oneshot::channel();
            self.started
                .send((context.id, complete))
                .expect("test receiver stays open");
            response
                .await
                .unwrap_or_else(|_| Err(ErrorData::internal_error("test response dropped", None)))
        }
        async fn handle_notification(
            &self,
            _: ClientNotification,
            _: NotificationContext<RoleServer>,
        ) -> Result<(), ErrorData> {
            Ok(())
        }
        fn get_info(&self) -> ServerConfig {
            ServerConfig::new(ServerCapabilities::default())
        }
    }

    #[tokio::test]
    async fn cancelled_wire_id_reuse_keeps_late_sdk_responses_separate() {
        let (client, server) = tokio::io::duplex(32 * 1024);
        let (client_read, mut client_write) = tokio::io::split(client);
        let (server_read, server_write) = tokio::io::split(server);
        let transport = StdioTransport::new(server_read, server_write);
        let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();
        let service = rmcp::service::serve_directly(
            DelayedService {
                started: started_tx,
            },
            transport,
            None,
        );
        let server_task = tokio::spawn(service.waiting());
        let mut client_read = tokio::io::BufReader::new(client_read);

        async fn put(writer: &mut (impl AsyncWrite + Unpin), value: Value) {
            writer
                .write_all(format!("{value}\n").as_bytes())
                .await
                .unwrap();
        }
        async fn response<R: tokio::io::AsyncRead + Unpin>(
            reader: &mut tokio::io::BufReader<R>,
        ) -> Value {
            let mut line = String::new();
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                reader.read_line(&mut line),
            )
            .await
            .expect("response arrives")
            .unwrap();
            serde_json::from_str(&line).unwrap()
        }
        fn ping(id: Value) -> Value {
            json!({
                "jsonrpc":"2.0",
                "id":id,
                "method":"ping",
                "params":{"_meta":{
                    "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                    "io.modelcontextprotocol/clientInfo":{"name":"test","version":"1"},
                    "io.modelcontextprotocol/clientCapabilities":{}
                }}
            })
        }

        put(&mut client_write, ping(json!(7))).await;
        let (internal_a, complete_a) = started_rx.recv().await.unwrap();
        assert_ne!(internal_a, RequestId::Number(7));
        put(
            &mut client_write,
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"unknown"}}),
        )
        .await;
        put(
            &mut client_write,
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}),
        )
        .await;
        put(&mut client_write, ping(json!(7))).await;
        let (internal_b, complete_b) = started_rx.recv().await.unwrap();
        assert_ne!(internal_a, internal_b);

        // A distinct string ID with the same display digits remains independent.
        put(&mut client_write, ping(json!("7"))).await;
        let (internal_string, complete_string) = started_rx.recv().await.unwrap();
        assert_ne!(internal_b, internal_string);
        complete_string.send(Ok(ServerResult::empty(()))).unwrap();
        assert_eq!(response(&mut client_read).await["id"], "7");

        // The SDK still finishes cancelled work, but it must drop A's old
        // internal ID instead of consuming B's newly admitted wire ID.
        complete_a.send(Ok(ServerResult::empty(()))).unwrap();
        let mut line = String::new();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(50),
                client_read.read_line(&mut line)
            )
            .await
            .is_err()
        );

        put(&mut client_write, ping(json!(7))).await;
        let error = response(&mut client_read).await;
        assert_eq!(error["id"], Value::Null);
        assert_eq!(error["error"]["code"], -32600);
        complete_b.send(Ok(ServerResult::empty(()))).unwrap();
        assert_eq!(response(&mut client_read).await["id"], 7);

        put(&mut client_write, ping(json!(7))).await;
        let (internal_c, complete_c) = started_rx.recv().await.unwrap();
        assert_ne!(internal_b, internal_c);
        complete_c.send(Ok(ServerResult::empty(()))).unwrap();
        assert_eq!(response(&mut client_read).await["id"], 7);

        drop(client_write);
        drop(client_read);
        tokio::time::timeout(std::time::Duration::from_secs(2), server_task)
            .await
            .expect("server exits after EOF")
            .unwrap()
            .unwrap();
    }
}

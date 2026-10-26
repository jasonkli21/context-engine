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
use std::{borrow::Cow, future::Future, io, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

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
    outstanding: Arc<std::sync::Mutex<std::collections::HashSet<RequestId>>>,
}
impl<R: tokio::io::AsyncRead, W> StdioTransport<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader: tokio::io::BufReader::new(reader),
            writer: Arc::new(tokio::sync::Mutex::new(writer)),
            line: Vec::new(),
            discarding: false,
            outstanding: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }
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
        let id = match &item {
            ServerJsonRpcMessage::Response(response) => Some(response.id.clone()),
            ServerJsonRpcMessage::Error(error) => error.id.clone(),
            _ => None,
        };
        async move {
            let mut bytes = serde_json::to_vec(&item).map_err(io::Error::other)?;
            bytes.push(b'\n');
            let mut writer = writer.lock().await;
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                writer.write_all(&bytes).await?;
                writer.flush().await
            })
            .await
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
            if let Some(id) = id {
                pending.lock().unwrap().remove(&id);
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
                Ok(message) => match &message {
                    ClientJsonRpcMessage::Request(request) => {
                        let rejection = {
                            let mut pending = self.outstanding.lock().unwrap();
                            if pending.contains(&request.id) {
                                // Null ID avoids correlating this rejection with the
                                // admitted original or releasing its response slot.
                                Some((None, "Duplicate outstanding request ID"))
                            } else if pending.len() >= 16 {
                                Some((
                                    Some(request.id.clone()),
                                    "Outstanding request limit reached",
                                ))
                            } else {
                                pending.insert(request.id.clone());
                                None
                            }
                        };
                        if let Some((id, reason)) = rejection {
                            let error =
                                JsonRpcError::new(id, ErrorData::invalid_request(reason, None));
                            if self.send(ServerJsonRpcMessage::Error(error)).await.is_err() {
                                return None;
                            }
                            continue;
                        }
                        return Some(message);
                    }
                    ClientJsonRpcMessage::Notification(notification)
                        if let ClientNotification::CancelledNotification(cancelled) =
                            &notification.notification =>
                    {
                        if let Some(id) = &cancelled.params.request_id {
                            self.outstanding.lock().unwrap().remove(id);
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
                    if self.send(message).await.is_err() {
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
        for id in 0..16 {
            put(
                &mut client,
                json!({"jsonrpc":"2.0", "id":id, "method":"ping"}),
            )
            .await;
        }
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
            json!({"jsonrpc":"2.0", "method":"notifications/cancelled", "params":{"requestId":15}}),
        )
        .await;
        for _ in 0..16 {
            assert!(matches!(
                transport.receive().await,
                Some(ClientJsonRpcMessage::Request(_))
            ));
        }
        assert_eq!(transport.outstanding.lock().unwrap().len(), 16);
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
        assert_eq!(transport.outstanding.lock().unwrap().len(), 15);
        assert!(
            transport
                .outstanding
                .lock()
                .unwrap()
                .contains(&RequestId::Number(0))
        );
        // An actual admitted response still flushes and releases exactly its slot.
        transport
            .send(ServerJsonRpcMessage::Error(JsonRpcError::new(
                Some(RequestId::Number(0)),
                ErrorData::invalid_params("fixture", None),
            )))
            .await
            .unwrap();
        let mut line = String::new();
        wire.read_line(&mut line).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 0);
        assert_eq!(transport.outstanding.lock().unwrap().len(), 14);
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
}

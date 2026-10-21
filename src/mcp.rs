//! MCP 2026-07-28 stdio binding through the maintained, pinned official SDK.
use crate::{
    adapter::{self, Request, Worker},
    application::Application,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::{NotificationContext, RequestContext, Service},
};
use std::{borrow::Cow, sync::Arc};

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
    Tool::new(
        name,
        description,
        Arc::new(schema.as_object().unwrap().clone()),
    )
    .with_annotations(
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
            "Full-file source context, file-ranker-v1; budget 1..1048576 UTF-8 bytes. Exposes source contents.",
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
        let request = Request::parse(&r.name, &bytes)
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
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
    let transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(tokio::io::stdin(), tokio::io::stdout())
            .with_max_line_length(adapter::MAX_REQUEST);
    // Modern-only service: skip the SDK's dual-era bootstrap, which otherwise
    // rejects notifications/EOF before the first non-discover request.
    let transport = BoundedTransport {
        inner: transport,
        outstanding: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
    };
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

// Backpressure also covers clients that send requests while refusing to read
// stdout. Fail closed rather than accumulating unlimited SDK response tasks.
struct BoundedTransport<T> {
    inner: T,
    outstanding: Arc<std::sync::Mutex<std::collections::HashSet<RequestId>>>,
}
impl<T: rmcp::transport::Transport<RoleServer, Error = std::io::Error> + 'static>
    rmcp::transport::Transport<RoleServer> for BoundedTransport<T>
{
    type Error = std::io::Error;
    fn send(
        &mut self,
        item: ServerJsonRpcMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let id = match &item {
            ServerJsonRpcMessage::Response(r) => Some(r.id.clone()),
            ServerJsonRpcMessage::Error(e) => e.id.clone(),
            _ => None,
        };
        let pending = self.outstanding.clone();
        let send = self.inner.send(item);
        async move {
            let result = tokio::time::timeout(std::time::Duration::from_secs(10), send)
                .await
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
            if let Some(id) = id {
                pending.lock().unwrap().remove(&id);
            }
            result
        }
    }
    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        let message = self.inner.receive().await?;
        if let ClientJsonRpcMessage::Request(r) = &message {
            let mut pending = self.outstanding.lock().unwrap();
            if pending.len() >= 16 || !pending.insert(r.id.clone()) {
                return None;
            }
        }
        Some(message)
    }
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.inner.close().await
    }
}

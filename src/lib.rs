//! # tiny-agent-protocol
//!
//! Minimal HTTP-like protocol for microcontrollers with 256-byte frames,
//! plain text serialization, and composable middleware.

use std::fmt;

/// Maximum frame size in bytes (fits ESP32 stack).
pub const MAX_FRAME: usize = 256;

/// Supported methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TAPMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

impl fmt::Display for TAPMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TAPMethod::GET => write!(f, "GET"),
            TAPMethod::POST => write!(f, "POST"),
            TAPMethod::PUT => write!(f, "PUT"),
            TAPMethod::DELETE => write!(f, "DELETE"),
        }
    }
}

/// A minimal HTTP request that fits in 256 bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct TAPRequest {
    pub method: TAPMethod,
    pub path: String,
    pub body: String,
}

impl TAPRequest {
    /// Create a new request.
    pub fn new(method: TAPMethod, path: &str, body: &str) -> Self {
        Self {
            method,
            path: path.to_string(),
            body: body.to_string(),
        }
    }

    /// Check whether the serialized form fits within MAX_FRAME.
    pub fn fits(&self) -> bool {
        self.serialized_len() <= MAX_FRAME
    }

    /// Compute the serialized length.
    pub fn serialized_len(&self) -> usize {
        format!("{} {}\n{}", self.method, self.path, self.body).len()
    }
}

/// A minimal HTTP response that fits in 256 bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct TAPResponse {
    pub status: u16,
    pub body: String,
}

impl TAPResponse {
    /// Create a new response.
    pub fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.to_string(),
        }
    }

    /// Convenience: 200 OK.
    pub fn ok(body: &str) -> Self {
        Self::new(200, body)
    }

    /// Convenience: 404 Not Found.
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }

    /// Convenience: 403 Forbidden.
    pub fn forbidden() -> Self {
        Self::new(403, "Forbidden")
    }

    /// Convenience: 429 Too Many Requests.
    pub fn rate_limited() -> Self {
        Self::new(429, "Too Many Requests")
    }

    /// Check whether the serialized form fits within MAX_FRAME.
    pub fn fits(&self) -> bool {
        self.serialized_len() <= MAX_FRAME
    }

    /// Compute the serialized length.
    pub fn serialized_len(&self) -> usize {
        format!("{}\n{}", self.status, self.body).len()
    }
}

/// Maximum number of routes (ESP32 memory constraint).
pub const MAX_ROUTES: usize = 16;

/// Handler function type: receives a request and returns a response.
pub type HandlerFn = fn(&TAPRequest) -> TAPResponse;

/// A route entry.
#[derive(Debug, Clone)]
pub struct Route {
    pub method: TAPMethod,
    pub path: String,
    pub handler: HandlerFn,
}

/// A lightweight router with at most 16 routes.
#[derive(Debug)]
pub struct TAPRouter {
    routes: Vec<Route>,
}

impl TAPRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Add a route. Returns `false` if the route table is full.
    pub fn add(&mut self, method: TAPMethod, path: &str, handler: HandlerFn) -> bool {
        if self.routes.len() >= MAX_ROUTES {
            return false;
        }
        self.routes.push(Route {
            method,
            path: path.to_string(),
            handler,
        });
        true
    }

    /// Dispatch a request to the matching route, or return 404.
    pub fn dispatch(&self, req: &TAPRequest) -> TAPResponse {
        for route in &self.routes {
            if route.method == req.method && route.path == req.path {
                return (route.handler)(req);
            }
        }
        TAPResponse::not_found()
    }

    /// Number of registered routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if the router has no routes.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

impl Default for TAPRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Plain-text serializer (no JSON dependency on microcontroller).
pub struct TAPSerializer;

impl TAPSerializer {
    /// Serialize a request to plain text: `METHOD path\nbody`.
    pub fn serialize_request(req: &TAPRequest) -> String {
        format!("{} {}\n{}", req.method, req.path, req.body)
    }

    /// Deserialize a plain text string into a request.
    /// Returns `None` if the format is invalid.
    pub fn deserialize_request(data: &str) -> Option<TAPRequest> {
        let first_line_end = data.find('\n')?;
        let first_line = &data[..first_line_end];
        let body = &data[first_line_end + 1..];
        let (method_str, path) = first_line.split_once(' ')?;
        let method = match method_str {
            "GET" => TAPMethod::GET,
            "POST" => TAPMethod::POST,
            "PUT" => TAPMethod::PUT,
            "DELETE" => TAPMethod::DELETE,
            _ => return None,
        };
        Some(TAPRequest {
            method,
            path: path.to_string(),
            body: body.to_string(),
        })
    }

    /// Serialize a response to plain text: `status\nbody`.
    pub fn serialize_response(res: &TAPResponse) -> String {
        format!("{}\n{}", res.status, res.body)
    }

    /// Deserialize a plain text string into a response.
    pub fn deserialize_response(data: &str) -> Option<TAPResponse> {
        let newline = data.find('\n')?;
        let status: u16 = data[..newline].parse().ok()?;
        let body = data[newline + 1..].to_string();
        Some(TAPResponse { status, body })
    }
}

/// Middleware function type: receives a request, returns `Some(response)` to
/// short-circuit, or `None` to continue to the next middleware / handler.
pub type MiddlewareFn = fn(&TAPRequest) -> Option<TAPResponse>;

/// A chain of composable middleware.
#[derive(Debug)]
pub struct TAPMiddleware {
    stack: Vec<MiddlewareFn>,
}

impl TAPMiddleware {
    /// Create empty middleware.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Add a middleware layer.
    pub fn add(&mut self, mw: MiddlewareFn) {
        self.stack.push(mw);
    }

    /// Run the middleware chain. Returns the first short-circuit response,
    /// or `None` if all middleware pass.
    pub fn run(&self, req: &TAPRequest) -> Option<TAPResponse> {
        for mw in &self.stack {
            if let Some(resp) = mw(req) {
                return Some(resp);
            }
        }
        None
    }

    /// Number of middleware layers.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Check if there are no middleware layers.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl Default for TAPMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

// --- Built-in middleware ---

/// Auth middleware: rejects requests whose body doesn't start with "token:".
pub fn auth_middleware(req: &TAPRequest) -> Option<TAPResponse> {
    if req.body.starts_with("token:") || req.path == "/health" {
        None
    } else {
        Some(TAPResponse::forbidden())
    }
}

/// Logging middleware: always passes (side-effect in real use).
/// Returns `None` to continue the chain.
pub fn logging_middleware(_req: &TAPRequest) -> Option<TAPResponse> {
    // In a real impl, this would log to serial.
    None
}

/// Rate-limit middleware: rejects if body contains "rate:exceeded".
pub fn rate_limit_middleware(req: &TAPRequest) -> Option<TAPResponse> {
    if req.body.contains("rate:exceeded") {
        Some(TAPResponse::rate_limited())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_fits_within_frame() {
        let req = TAPRequest::new(TAPMethod::GET, "/hello", "world");
        assert!(req.fits());
    }

    #[test]
    fn request_exceeds_frame() {
        let body = "x".repeat(300);
        let req = TAPRequest::new(TAPMethod::POST, "/data", &body);
        assert!(!req.fits());
    }

    #[test]
    fn response_ok() {
        let res = TAPResponse::ok("done");
        assert_eq!(res.status, 200);
        assert_eq!(res.body, "done");
    }

    #[test]
    fn response_not_found() {
        let res = TAPResponse::not_found();
        assert_eq!(res.status, 404);
    }

    #[test]
    fn router_dispatch_match() {
        fn hello(_req: &TAPRequest) -> TAPResponse {
            TAPResponse::ok("hello!")
        }
        let mut router = TAPRouter::new();
        assert!(router.add(TAPMethod::GET, "/hello", hello));
        let req = TAPRequest::new(TAPMethod::GET, "/hello", "");
        let res = router.dispatch(&req);
        assert_eq!(res.status, 200);
        assert_eq!(res.body, "hello!");
    }

    #[test]
    fn router_dispatch_not_found() {
        let router = TAPRouter::new();
        let req = TAPRequest::new(TAPMethod::GET, "/missing", "");
        let res = router.dispatch(&req);
        assert_eq!(res.status, 404);
    }

    #[test]
    fn router_max_routes() {
        let mut router = TAPRouter::new();
        fn noop(_req: &TAPRequest) -> TAPResponse { TAPResponse::ok("") }
        for i in 0..MAX_ROUTES {
            assert!(router.add(TAPMethod::GET, &format!("/r{}", i), noop));
        }
        assert!(!router.add(TAPMethod::GET, "/overflow", noop));
    }

    #[test]
    fn serialize_deserialize_request() {
        let req = TAPRequest::new(TAPMethod::POST, "/data", "hello");
        let ser = TAPSerializer::serialize_request(&req);
        let de = TAPSerializer::deserialize_request(&ser).unwrap();
        assert_eq!(de.method, TAPMethod::POST);
        assert_eq!(de.path, "/data");
        assert_eq!(de.body, "hello");
    }

    #[test]
    fn deserialize_invalid_request() {
        assert!(TAPSerializer::deserialize_request("INVALID").is_none());
    }

    #[test]
    fn serialize_deserialize_response() {
        let res = TAPResponse::ok("world");
        let ser = TAPSerializer::serialize_response(&res);
        let de = TAPSerializer::deserialize_response(&ser).unwrap();
        assert_eq!(de.status, 200);
        assert_eq!(de.body, "world");
    }

    #[test]
    fn deserialize_invalid_response() {
        assert!(TAPSerializer::deserialize_response("not_a_number").is_none());
    }

    #[test]
    fn middleware_chain_passes() {
        let mut mw = TAPMiddleware::new();
        mw.add(logging_middleware);
        let req = TAPRequest::new(TAPMethod::GET, "/test", "ok");
        assert!(mw.run(&req).is_none());
    }

    #[test]
    fn middleware_chain_short_circuits() {
        let mut mw = TAPMiddleware::new();
        mw.add(auth_middleware);
        let req = TAPRequest::new(TAPMethod::GET, "/secret", "no-token");
        let result = mw.run(&req);
        assert!(result.is_some());
        assert_eq!(result.unwrap().status, 403);
    }

    #[test]
    fn auth_middleware_allows_token() {
        let req = TAPRequest::new(TAPMethod::GET, "/secret", "token:abc");
        assert!(auth_middleware(&req).is_none());
    }

    #[test]
    fn auth_middleware_allows_health() {
        let req = TAPRequest::new(TAPMethod::GET, "/health", "");
        assert!(auth_middleware(&req).is_none());
    }

    #[test]
    fn rate_limit_middleware_rejects() {
        let req = TAPRequest::new(TAPMethod::GET, "/", "rate:exceeded");
        let res = super::rate_limit_middleware(&req).unwrap();
        assert_eq!(res.status, 429);
    }

    #[test]
    fn method_display() {
        assert_eq!(format!("{}", TAPMethod::GET), "GET");
        assert_eq!(format!("{}", TAPMethod::POST), "POST");
    }
}

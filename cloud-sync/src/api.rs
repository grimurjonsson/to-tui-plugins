//! Cloud API client.
//!
//! HTTP client for communicating with the totui cloud sync API.
//! Handles authentication, project management, and todo synchronization.

use crate::models::*;

/// Cloud API client.
pub struct CloudClient {
    api_url: String,
    api_key: String,
}

impl CloudClient {
    /// Create a new cloud client.
    pub fn new(api_url: &str, api_key: &str) -> Self {
        // Normalize URL: strip trailing slash
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            api_key: api_key.to_string(),
        }
    }

    /// Build a full URL for an API endpoint.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_url, path)
    }

    /// Create an authenticated request.
    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let url = self.url(path);
        let req = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            "DELETE" => ureq::delete(&url),
            _ => ureq::get(&url),
        };
        req.set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
    }

    /// Verify that the API key is valid.
    pub fn verify_auth(&self) -> Result<bool, ApiClientError> {
        let resp = self
            .request("GET", "/api/v1/auth/verify")
            .call()
            .map_err(|e| ApiClientError::from_ureq(e))?;
        Ok(resp.status() == 200)
    }

    /// List all projects for the authenticated user.
    pub fn list_projects(&self) -> Result<Vec<CloudProject>, ApiClientError> {
        let resp = self
            .request("GET", "/api/v1/projects")
            .call()
            .map_err(|e| ApiClientError::from_ureq(e))?;

        resp.into_json::<Vec<CloudProject>>()
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }

    /// Create a new project.
    pub fn create_project(&self, name: &str) -> Result<CloudProject, ApiClientError> {
        let body = CreateProjectRequest {
            name: name.to_string(),
        };

        let resp = self
            .request("POST", "/api/v1/projects")
            .send_json(&body)
            .map_err(|e| ApiClientError::from_ureq(e))?;

        resp.into_json::<CloudProject>()
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }

    /// Get all todos for a project.
    pub fn get_todos(&self, project_id: &str) -> Result<Vec<CloudTodo>, ApiClientError> {
        let path = format!("/api/v1/projects/{}/todos", project_id);
        let resp = self
            .request("GET", &path)
            .call()
            .map_err(|e| ApiClientError::from_ureq(e))?;

        resp.into_json::<Vec<CloudTodo>>()
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }

    /// Sync todos with the cloud.
    ///
    /// Pushes local state and receives remote changes.
    /// The server handles merge/conflict resolution using last-write-wins.
    pub fn sync_todos(
        &self,
        project_id: &str,
        request: &SyncRequest,
    ) -> Result<SyncResponse, ApiClientError> {
        let path = format!("/api/v1/projects/{}/sync", project_id);
        let resp = self
            .request("POST", &path)
            .send_json(request)
            .map_err(|e| ApiClientError::from_ureq(e))?;

        resp.into_json::<SyncResponse>()
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }

    /// Update a single todo.
    pub fn update_todo(&self, todo: &CloudTodo) -> Result<CloudTodo, ApiClientError> {
        let path = format!("/api/v1/todos/{}", todo.id);
        let resp = self
            .request("PUT", &path)
            .send_json(todo)
            .map_err(|e| ApiClientError::from_ureq(e))?;

        resp.into_json::<CloudTodo>()
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }

    /// Delete a todo.
    pub fn delete_todo(&self, todo_id: &str) -> Result<(), ApiClientError> {
        let path = format!("/api/v1/todos/{}", todo_id);
        self.request("DELETE", &path)
            .call()
            .map_err(|e| ApiClientError::from_ureq(e))?;
        Ok(())
    }
}

// ============================================================================
// Error handling
// ============================================================================

/// API client error types.
#[derive(Debug)]
pub enum ApiClientError {
    /// Network or connection error
    Network(String),
    /// Server returned an error status
    Server { status: u16, message: String },
    /// Failed to parse response
    Parse(String),
    /// Authentication failed
    Unauthorized,
}

impl ApiClientError {
    /// Convert a ureq error into our error type.
    fn from_ureq(err: ureq::Error) -> Self {
        match err {
            ureq::Error::Status(status, response) => {
                if status == 401 || status == 403 {
                    return ApiClientError::Unauthorized;
                }
                let message = response
                    .into_string()
                    .unwrap_or_else(|_| "Unknown error".to_string());
                ApiClientError::Server { status, message }
            }
            ureq::Error::Transport(transport) => {
                ApiClientError::Network(transport.to_string())
            }
        }
    }
}

impl std::fmt::Display for ApiClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiClientError::Network(msg) => write!(f, "Network error: {}", msg),
            ApiClientError::Server { status, message } => {
                write!(f, "Server error ({}): {}", status, message)
            }
            ApiClientError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ApiClientError::Unauthorized => write!(f, "Authentication failed - check your API key"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_normalization() {
        let client = CloudClient::new("https://api.example.com/", "test-key");
        assert_eq!(
            client.url("/api/v1/projects"),
            "https://api.example.com/api/v1/projects"
        );

        let client2 = CloudClient::new("https://api.example.com", "test-key");
        assert_eq!(
            client2.url("/api/v1/projects"),
            "https://api.example.com/api/v1/projects"
        );
    }
}

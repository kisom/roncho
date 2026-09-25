use roncho::Honcho;

pub fn make_client(server_url: &str) -> Honcho {
    Honcho::builder()
        .workspace_id("test-workspace")
        .api_key("test-key")
        .base_url(server_url)
        .build()
        .expect("failed to build client")
}

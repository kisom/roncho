use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub size: usize,
    pub pages: usize,
}

impl<T> Page<T> {
    pub fn has_next(&self) -> bool {
        self.page < self.pages
    }

    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}

/// Options for paginated list requests.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    pub page: Option<usize>,
    pub size: Option<usize>,
    pub reverse: Option<bool>,
}

impl ListOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn page(mut self, page: usize) -> Self {
        self.page = Some(page);
        self
    }

    pub fn size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }
}

pub mod config;
pub mod output;
pub mod pipeline;
pub mod segment;
pub mod pack;
pub mod clean;
pub mod token;

pub use config::{FragmentConfig, Segmenter, Packer, CleanProfile, CleanFlags};
pub use output::{Fragment, FragmentMeta, FragmenterResult, FragmenterPair, FragmenterPairedResult};
pub use pipeline::Fragmenter;

// Backward-compatibility aliases
pub type FragmentOptions = FragmentConfig;
pub type FragmentMode = CleanProfile;
pub use clean::tables::TableInfo;

pub fn fragment_paired(content: String, options: &FragmentConfig) -> FragmenterPairedResult {
    let fragmenter = Fragmenter::new();
    fragmenter.run_paired_sync(&content, options)
}

pub fn fragment_for_embedding(content: &str, options: &FragmentConfig) -> Vec<String> {
    let fragmenter = Fragmenter::new();
    let config = FragmentConfig {
        segmenter: options.segmenter.clone(),
        packer: options.packer.clone(),
        cleaner: CleanProfile::Embedding,
        include_heading_in_fragments: options.include_heading_in_fragments,
        extract_frontmatter: options.extract_frontmatter,
    };
    let res = fragmenter.run_sync(content, &config);
    res.fragments.into_iter().map(|f| f.text).collect()
}

pub fn fragment_for_generation(content: &str, options: &FragmentConfig) -> Vec<String> {
    let fragmenter = Fragmenter::new();
    let config = FragmentConfig {
        segmenter: options.segmenter.clone(),
        packer: options.packer.clone(),
        cleaner: CleanProfile::Generation,
        include_heading_in_fragments: options.include_heading_in_fragments,
        extract_frontmatter: options.extract_frontmatter,
    };
    let res = fragmenter.run_sync(content, &config);
    res.fragments.into_iter().map(|f| f.text).collect()
}

#[async_trait::async_trait]
pub trait Chunker {
    async fn chunk(
        &self,
        content: &str,
        llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk>;
}

pub struct RuleChunker {
    pub max_chars: usize,
}

impl RuleChunker {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

#[async_trait::async_trait]
impl Chunker for RuleChunker {
    async fn chunk(
        &self,
        content: &str,
        _llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk> {
        let config = FragmentConfig::topic(self.max_chars);
        let fragmenter = Fragmenter::new();
        let result = fragmenter.run(content, &config).await;
        result
            .fragments
            .into_iter()
            .map(|f| crate::domain::distribute::TopicChunk {
                text: f.text,
                suggested_title: f.meta.suggested_title.unwrap_or_default(),
            })
            .collect()
    }
}

pub struct SemanticChunker {
    pub max_chars: usize,
    pub threshold: f32,
}

impl SemanticChunker {
    pub fn new(max_chars: usize, threshold: f32) -> Self {
        Self { max_chars, threshold }
    }
}

#[async_trait::async_trait]
impl Chunker for SemanticChunker {
    async fn chunk(
        &self,
        content: &str,
        llm_service: &std::sync::Arc<crate::ai::LlmService>,
    ) -> Vec<crate::domain::distribute::TopicChunk> {
        let config = FragmentConfig::topic_semantic(self.max_chars, self.threshold);
        let fragmenter = Fragmenter::with_llm(llm_service.clone());
        let result = fragmenter.run(content, &config).await;
        result
            .fragments
            .into_iter()
            .map(|f| crate::domain::distribute::TopicChunk {
                text: f.text,
                suggested_title: f.meta.suggested_title.unwrap_or_default(),
            })
            .collect()
    }
}

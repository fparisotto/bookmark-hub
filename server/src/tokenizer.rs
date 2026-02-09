use cantrip::*;
use tokenizers::tokenizer::Tokenizer;

const TOKENIZER_MODEL: &str = "bert-base-cased";

/// Count tokens in text using BERT tokenizer
pub fn count_tokens(text: &str) -> anyhow::Result<usize> {
    let tokenizer =
        Tokenizer::from_pretrained(TOKENIZER_MODEL, None).map_err(anyhow::Error::from_boxed)?;
    let encoding = tokenizer
        .encode(text, false)
        .map_err(anyhow::Error::from_boxed)?;
    Ok(encoding.get_tokens().len())
}

pub fn windowed_chunks(
    size: usize,
    edge_overlap: usize,
    text: &str,
) -> anyhow::Result<Vec<String>> {
    let tokenizer =
        Tokenizer::from_pretrained(TOKENIZER_MODEL, None).map_err(anyhow::Error::from_boxed)?;
    let encoding = tokenizer
        .encode(text, false)
        .map_err(anyhow::Error::from_boxed)?;
    let ids: Vec<_> = encoding.get_ids().to_vec();
    let windowed = ids.windowed(size, size - edge_overlap);
    let mut result = Vec::new();
    for window in windowed {
        let window_ids: Vec<u32> = window.to_vec();
        let window_text = tokenizer
            .decode(&window_ids, true)
            .map_err(anyhow::Error::from_boxed)?;
        result.push(window_text);
    }
    Ok(result)
}

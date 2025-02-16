use cantrip::*;
use tokenizers::tokenizer::Tokenizer;

const TOKENIZER_MODEL: &str = "bert-base-cased";

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
    let tokens: Vec<_> = encoding.get_tokens().iter().collect();
    let windowed = tokens.windowed(size, size - edge_overlap);
    let mut result = Vec::new();
    for window in windowed {
        let window_text = window
            .iter()
            .map(|e| (*e).to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        result.push(window_text);
    }
    Ok(result)
}

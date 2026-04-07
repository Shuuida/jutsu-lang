use super::{Evaluator, VesselConfig};

impl Evaluator {
    pub fn native_vector_search(&self, query: &str, document: &str) -> String {
        let delimiters = ['.', '\n', '!', '?'];
        let mut sentences = Vec::new();
        let mut current_sentence = String::new();
        for c in document.chars() {
            current_sentence.push(c);
            if delimiters.contains(&c) {
                let trimmed = current_sentence.trim().to_string();
                if !trimmed.is_empty() { sentences.push(trimmed); }
                current_sentence.clear();
            }
        }
        if !current_sentence.trim().is_empty() { sentences.push(current_sentence.trim().to_string()); }

        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        for sentence in sentences {
            if current_chunk.len() + sentence.len() > 400 && !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk.clear();
            }
            current_chunk.push_str(&sentence);
            current_chunk.push(' ');
        }
        if !current_chunk.trim().is_empty() { chunks.push(current_chunk.trim().to_string()); }
        
        let clean_query = query.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
        let query_tokens: Vec<String> = clean_query.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let mut best_chunk = String::new();
        let mut highest_cosine = -1.0_f32;

        for chunk in &chunks {
            let clean_chunk = chunk.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
            let chunk_tokens: Vec<String> = clean_chunk.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
            let mut dot_product = 0.0;
            for qt in &query_tokens { if chunk_tokens.contains(qt) { dot_product += 1.0; } }
            let magnitude_q = (query_tokens.len() as f32).sqrt();
            let magnitude_c = (chunk_tokens.len() as f32).sqrt();
            let cosine_sim = if magnitude_q > 0.0 && magnitude_c > 0.0 { dot_product / (magnitude_q * magnitude_c) } else { 0.0 };
            if cosine_sim > highest_cosine { highest_cosine = cosine_sim; best_chunk = chunk.clone(); }
        }
        best_chunk
    }

    pub fn execute_infer(&self, _name: &str, config: &VesselConfig, raw_prompt: &str, context_text: Option<String>) -> Option<String> {
        crate::inference::run_inference(
            config.model_ptr, 
            config.quantize, 
            config.temp, 
            config.bind, 
            raw_prompt, 
            context_text,
            None
        )
    }
}
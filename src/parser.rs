use anyhow::Result;
use ndarray::Array2;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Value;
use regex::Regex;
use std::path::Path;
use tokenizers::Tokenizer;

pub struct ExpenseParser {
    session: Session,
    tokenizer: Tokenizer,
    number_regex: Regex,
}

#[derive(Debug)]
pub struct ParsedExpense {
    pub intent: String,
    pub language: String,
    pub amount: f64,
    pub description: String,
    pub item_name: String,
    pub category_group: String,
}

impl ExpenseParser {
    pub fn new(model_path: impl AsRef<Path>, tokenizer_path: impl AsRef<Path>) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .with_intra_threads(1)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let number_regex = Regex::new(r"(?i)(?:rp|\$)?\s*(\d+(?:[.,]\d+)?)\s*(rb|ribu|rebu|k|jt|juta|jeti|grand|bucks|mil|m)?")?;

        Ok(Self { session, tokenizer,  number_regex})
    }

    pub fn parse(&mut self, text: &str) -> Result<ParsedExpense> {
        // --- TAHAP 1: TOKENISASI ---
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization error: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();

        let seq_len = input_ids.len();

        // KUNCI PERBAIKAN: Gunakan ndarray::Array2 untuk membungkus Vec menjadi matriks 2D
        let input_ids_array = Array2::from_shape_vec((1, seq_len), input_ids)
            .map_err(|e| anyhow::anyhow!("Shape error input_ids: {}", e))?;

        let attention_mask_array = Array2::from_shape_vec((1, seq_len), attention_mask)
            .map_err(|e| anyhow::anyhow!("Shape error attention_mask: {}", e))?;

        // Masukkan Array2 ke dalam ort::Value
        let input_ids_tensor = Value::from_array(input_ids_array)?;
        let attention_mask_tensor = Value::from_array(attention_mask_array)?;

        // --- TAHAP 2: INFERENSI KE MODEL ONNX ---
        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
        ])?;

        // --- 3. EKSTRAK INTENT (outputs[0]) ---
        // Pecah tuple, abaikan shape (_), ambil datanya (intent_data)
        let (_, intent_data) = outputs[0].try_extract_tensor::<f32>()?;
        let intent_idx = Self::argmax(intent_data);
        let intent = match intent_idx {
            0 => "add_expense",
            1 => "query",
            2 => "greeting",
            _ => "non_expense",
        };

        // --- 4. EKSTRAK BAHASA (outputs[1]) ---
        let (_, lang_data) = outputs[1].try_extract_tensor::<f32>()?;
        let lang_idx = Self::argmax(lang_data);
        let language = if lang_idx == 0 { "en" } else { "id" };

        let (_, category_data) = outputs[2].try_extract_tensor::<f32>()?;
        let category_idx = Self::argmax(category_data);
        let category_group = match category_idx {
            0 => "entertainment",
            1 => "food",
            2 => "transport",
            3 => "other",
            4 => "bills",
            5 => "shopping",
            6 => "health",
            _ => "unknown",
        };

        // --- 5 & 6. EKSTRAK NOMINAL & BERSIHKAN DESKRIPSI SECARA DETERMINISTIK ---
        let mut amount = 0.0;
        // LAPIS 1: Translasi Slang Absolut (Indo & English)
        let preprocessed_text = text.to_lowercase()
            // Slang Indo
            .replace("gocap", "50000")
            .replace("ceban", "10000")
            .replace("goceng", "5000")
            .replace("seceng", "1000")
            .replace("gopek", "500")
            .replace("cepek", "100")
            // Slang English
            .replace("a fiver", "5")
            .replace("a tenner", "10");

        let mut description = preprocessed_text.clone();

        // LAPIS 2: Tangkap Angka dan Suffix Pengali
        if let Some(caps) = self.number_regex.captures(&preprocessed_text) {
            let num_str = caps.get(1).unwrap().as_str().replace(",", ".");
            let mut parsed_num: f64 = num_str.parse().unwrap_or(0.0);

            if let Some(multiplier) = caps.get(2) {
                // Kalikan berdasarkan slang dari kedua bahasa
                match multiplier.as_str() {
                    "rb" | "ribu" | "rebu" | "k" | "grand" => parsed_num *= 1_000.0,
                    "jt" | "juta" | "jeti" | "mil" | "m" => parsed_num *= 1_000_000.0,
                    "bucks" => parsed_num *= 1.0, // Bucks tidak mengubah nilai, "50 bucks" = 50
                    _ => {}
                }
            }
            amount = parsed_num;

            let matched_text = caps.get(0).unwrap().as_str();
            description = preprocessed_text.replace(matched_text, "");
        }

        let description = description.split_whitespace().collect::<Vec<_>>().join(" ");

        // JIKA INTENT BUKAN ADD_EXPENSE, NOL-KAN AMOUNT
        if intent != "add_expense" {
            amount = 0.0;
        }

        Ok(ParsedExpense {
            intent: intent.to_string(),
            language: language.to_string(),
            amount,
            description: description.clone(),
            item_name: description,
            category_group: category_group.to_string(),
        })
    }

    fn argmax(slice: &[f32]) -> usize {
        slice.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

}

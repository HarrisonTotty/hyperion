//! Alien language generation
//!
//! This module generates basic procedural alien languages with phonology and vocabulary.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A procedurally generated alien language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlienLanguage {
    /// Language name
    pub name: String,
    /// Phonology (allowed sounds)
    pub phonology: Phonology,
    /// Word structure rules
    pub structure: WordStructure,
    /// Basic vocabulary
    pub vocabulary: HashMap<String, String>,
}

/// Phonology system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phonology {
    /// Consonants
    pub consonants: Vec<String>,
    /// Vowels
    pub vowels: Vec<String>,
    /// Allows consonant clusters
    pub consonant_clusters: bool,
    /// Allows final consonants
    pub final_consonants: bool,
}

/// Word structure rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordStructure {
    /// Minimum syllables per word
    pub min_syllables: usize,
    /// Maximum syllables per word
    pub max_syllables: usize,
    /// Syllable pattern (C=consonant, V=vowel)
    pub pattern: SyllablePattern,
}

/// Syllable pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyllablePattern {
    /// CV (consonant-vowel)
    CV,
    /// CVC (consonant-vowel-consonant)
    CVC,
    /// V (vowel only)
    V,
    /// VC (vowel-consonant)
    VC,
}

impl AlienLanguage {
    /// Generate a new alien language
    pub fn generate(name: String, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        
        let phonology = Self::generate_phonology(&mut rng);
        let structure = Self::generate_structure(&mut rng);
        let vocabulary = Self::generate_vocabulary(&mut rng, &phonology, &structure);
        
        AlienLanguage {
            name,
            phonology,
            structure,
            vocabulary,
        }
    }
    
    fn generate_phonology(rng: &mut StdRng) -> Phonology {
        // Common consonants
        let all_consonants = vec![
            "p", "t", "k", "b", "d", "g", "m", "n",
            "f", "s", "h", "v", "z", "l", "r", "w", "y",
            "ch", "sh", "th", "zh", "kh", "gh",
        ];
        
        // Select subset of consonants
        let num_consonants = rng.gen_range(8..16);
        let mut consonants = Vec::new();
        for _ in 0..num_consonants {
            let idx = rng.gen_range(0..all_consonants.len());
            let c = all_consonants[idx].to_string();
            if !consonants.contains(&c) {
                consonants.push(c);
            }
        }
        
        // Common vowels
        let all_vowels = vec!["a", "e", "i", "o", "u", "ae", "ai", "au", "ei", "ou"];
        
        // Select subset of vowels
        let num_vowels = rng.gen_range(3..7);
        let mut vowels = Vec::new();
        for _ in 0..num_vowels {
            let idx = rng.gen_range(0..all_vowels.len());
            let v = all_vowels[idx].to_string();
            if !vowels.contains(&v) {
                vowels.push(v);
            }
        }
        
        Phonology {
            consonants,
            vowels,
            consonant_clusters: rng.gen_bool(0.5),
            final_consonants: rng.gen_bool(0.7),
        }
    }
    
    fn generate_structure(rng: &mut StdRng) -> WordStructure {
        let min_syllables = rng.gen_range(1..=2);
        let max_syllables = rng.gen_range(min_syllables..=4);
        
        let pattern = match rng.gen_range(0..4) {
            0 => SyllablePattern::CV,
            1 => SyllablePattern::CVC,
            2 => SyllablePattern::V,
            _ => SyllablePattern::VC,
        };
        
        WordStructure {
            min_syllables,
            max_syllables,
            pattern,
        }
    }
    
    fn generate_vocabulary(rng: &mut StdRng, phonology: &Phonology, structure: &WordStructure) -> HashMap<String, String> {
        let mut vocabulary = HashMap::new();
        
        // Core vocabulary items
        let core_words = vec![
            "hello", "goodbye", "yes", "no", "please", "thank you",
            "friend", "enemy", "ship", "star", "planet", "station",
            "trade", "war", "peace", "alliance", "attack", "defend",
            "captain", "crew", "weapon", "shield", "engine",
        ];
        
        for word in core_words {
            let alien_word = Self::generate_word(rng, phonology, structure);
            vocabulary.insert(word.to_string(), alien_word);
        }
        
        vocabulary
    }
    
    fn generate_word(rng: &mut StdRng, phonology: &Phonology, structure: &WordStructure) -> String {
        let num_syllables = rng.gen_range(structure.min_syllables..=structure.max_syllables);
        let mut word = String::new();
        
        for i in 0..num_syllables {
            let syllable = Self::generate_syllable(rng, phonology, structure.pattern, i == num_syllables - 1);
            word.push_str(&syllable);
        }
        
        word
    }
    
    fn generate_syllable(rng: &mut StdRng, phonology: &Phonology, pattern: SyllablePattern, is_final: bool) -> String {
        let mut syllable = String::new();
        
        match pattern {
            SyllablePattern::CV => {
                // Consonant + Vowel
                let c = &phonology.consonants[rng.gen_range(0..phonology.consonants.len())];
                let v = &phonology.vowels[rng.gen_range(0..phonology.vowels.len())];
                syllable.push_str(c);
                syllable.push_str(v);
            }
            SyllablePattern::CVC => {
                // Consonant + Vowel + Consonant
                let c1 = &phonology.consonants[rng.gen_range(0..phonology.consonants.len())];
                let v = &phonology.vowels[rng.gen_range(0..phonology.vowels.len())];
                syllable.push_str(c1);
                syllable.push_str(v);
                
                if phonology.final_consonants || !is_final {
                    let c2 = &phonology.consonants[rng.gen_range(0..phonology.consonants.len())];
                    syllable.push_str(c2);
                }
            }
            SyllablePattern::V => {
                // Vowel only
                let v = &phonology.vowels[rng.gen_range(0..phonology.vowels.len())];
                syllable.push_str(v);
            }
            SyllablePattern::VC => {
                // Vowel + Consonant
                let v = &phonology.vowels[rng.gen_range(0..phonology.vowels.len())];
                syllable.push_str(v);
                
                if phonology.final_consonants || !is_final {
                    let c = &phonology.consonants[rng.gen_range(0..phonology.consonants.len())];
                    syllable.push_str(c);
                }
            }
        }
        
        syllable
    }
    
    /// Translate an English word to the alien language
    pub fn translate(&self, word: &str) -> Option<&String> {
        self.vocabulary.get(word)
    }
    
    /// Generate a random phrase
    pub fn generate_phrase(&self, seed: u64) -> String {
        let mut rng = StdRng::seed_from_u64(seed);
        let num_words = rng.gen_range(2..=5);
        let mut phrase = Vec::new();
        
        for _ in 0..num_words {
            let word = Self::generate_word(&mut rng, &self.phonology, &self.structure);
            phrase.push(word);
        }
        
        phrase.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_language_generation() {
        let language = AlienLanguage::generate("Klingon".to_string(), 42);
        
        assert_eq!(language.name, "Klingon");
        assert!(!language.phonology.consonants.is_empty());
        assert!(!language.phonology.vowels.is_empty());
        assert!(!language.vocabulary.is_empty());
    }
    
    #[test]
    fn test_vocabulary() {
        let language = AlienLanguage::generate("Test".to_string(), 123);
        
        // Should have core vocabulary
        assert!(language.vocabulary.contains_key("hello"));
        assert!(language.vocabulary.contains_key("ship"));
        assert!(language.vocabulary.contains_key("war"));
    }
    
    #[test]
    fn test_translation() {
        let language = AlienLanguage::generate("Test".to_string(), 456);
        
        let translation = language.translate("hello");
        assert!(translation.is_some());
        assert!(!translation.unwrap().is_empty());
    }
    
    #[test]
    fn test_phrase_generation() {
        let language = AlienLanguage::generate("Test".to_string(), 789);
        
        let phrase = language.generate_phrase(100);
        assert!(!phrase.is_empty());
        assert!(phrase.contains(' ')); // Multi-word phrase
    }
    
    #[test]
    fn test_phonology_constraints() {
        let language = AlienLanguage::generate("Test".to_string(), 321);
        
        // Consonants and vowels should be reasonable sizes
        assert!(language.phonology.consonants.len() >= 8);
        assert!(language.phonology.consonants.len() <= 16);
        assert!(language.phonology.vowels.len() >= 3);
        assert!(language.phonology.vowels.len() <= 7);
    }
    
    #[test]
    fn test_deterministic_generation() {
        let lang1 = AlienLanguage::generate("Test".to_string(), 42);
        let lang2 = AlienLanguage::generate("Test".to_string(), 42);
        
        // Same seed should produce same language
        assert_eq!(lang1.phonology.consonants, lang2.phonology.consonants);
        assert_eq!(lang1.phonology.vowels, lang2.phonology.vowels);
        
        let hello1 = lang1.translate("hello");
        let hello2 = lang2.translate("hello");
        assert_eq!(hello1, hello2);
    }
}

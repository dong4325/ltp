use crate::perceptron::definition::GenericItem;
use crate::perceptron::{Definition, Sample};
use crate::buf_feature;
use anyhow::Result;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "serialization")]
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct POSDefinition {
    to_labels: Vec<String>,
    labels_to: HashMap<String, usize>,
}

impl POSDefinition {
    pub fn new(to_labels: Vec<String>) -> Self {
        let labels_to = to_labels
            .iter()
            .enumerate()
            .map(|(i, label)| (label.clone(), i))
            .collect();
        POSDefinition {
            labels_to,
            to_labels,
        }
    }

    /// +----------------------+----------------------------------------------------------+
    // | 类别                 | 特征                                                       |
    // +======================+===========================================================+
    // | word-unigram         | w[-2],w[-1],w[0],w[1],w[2]                                |
    // +----------------------+-----------------------------------------------------------+
    // | word-bigram          | w[-2]w[-1],w[-1]w[0],w[0]w[1],w[1]w[2],w[-2]w[0],w[0]w[2] |
    // +----------------------+-----------------------------------------------------------+
    // | word-trigram         | w[-1]w[0]w[1]                                             |
    // +----------------------+-----------------------------------------------------------+
    // | last-first-character | ch[0,0]ch[0,n],ch[-1,n]ch[0,0],ch[0,-1]ch[1,0]            |
    // +----------------------+-----------------------------------------------------------+
    // | length               | length                                                    |
    // +----------------------+-----------------------------------------------------------+
    // | prefix               | ch[0,0],ch[0,0:1],ch[0,0:2]                               |
    // +----------------------+-----------------------------------------------------------+
    // | suffix               | ch[0,n-2:n],ch[0,n-1:n],ch[0,n]                           |
    // +----------------------+-----------------------------------------------------------+
    pub fn parse_words_features(&self, words: &[&str]) -> Vec<Vec<String>> {
        let word_null = "";
        let words_len = words.len();
        let mut features = Vec::with_capacity(words_len);

        let chars = words
            .iter()
            .map(|w| SmallVec::<[char; 4]>::from_iter(w.chars()))
            .collect_vec();

        for (idx, &cur_word) in words.iter().enumerate() {
            // 剩余字符数
            let last = words_len - idx - 1;
            let pre2_word = if idx > 1 { words[idx - 2] } else { word_null };
            let pre_word = if idx > 0 { words[idx - 1] } else { word_null };
            let next_word = if last > 0 { words[idx + 1] } else { word_null };
            let next2_word = if last > 1 { words[idx + 2] } else { word_null };

            // todo: 优化容量设置
            let mut feature = Vec::with_capacity(22);

            // w[0]
            feature.push(format!("2{}", words[idx]));
            // ch[0,0]ch[0,n]
            feature.push(format!(
                "c{}{}",
                chars[idx][0],
                chars[idx][chars[idx].len() - 1]
            ));
            // length
            feature.push(format!("f{}", chars[idx].len()));
            // prefix => ch[0,0]ch[0,0:1]ch[0,0:2]

            let prefix_id = &['c', 'd', 'e'];
            chars[idx]
                .iter()
                .take(3)
                .enumerate()
                .for_each(|(bias, prefix)| {
                    feature.push(format!("{}{}", prefix_id[bias], prefix));
                });
            // suffix => ch[0,n-2:n],ch[0,n-1:n],ch[0,n]
            let suffix_id = &['f', 'g', 'h'];
            chars[idx]
                .iter()
                .rev()
                .take(3)
                .enumerate()
                .for_each(|(bias, suffix)| {
                    feature.push(format!("{}{}", suffix_id[bias], suffix));
                });

            if idx > 0 {
                feature.push(format!("1{}", pre_word)); // w[-1]
                feature.push(format!("6{}{}", pre_word, cur_word)); // w[-1]w[0]
                feature.push(format!(
                    // ch[-1,n]ch[0,0]
                    "d{}{}",
                    chars[idx - 1][chars[idx - 1].len() - 1],
                    chars[idx][0]
                ));

                if idx > 1 {
                    feature.push(format!("0{}", pre2_word)); // w[-2]
                    feature.push(format!("5{}{}", pre2_word, pre_word)); // w[-2]w[-1]
                    feature.push(format!("9{}{}", pre2_word, cur_word)); // w[-2]w[0]
                }
            }

            if last > 0 {
                feature.push(format!("3{}", next_word)); // w[+1]
                feature.push(format!("7{}{}", cur_word, next_word)); // w[0]w[+1]
                feature.push(format!(
                    // ch[0,-1]ch[1,0]
                    "e{}{}",
                    chars[idx][chars[idx].len() - 1],
                    chars[idx + 1][0],
                ));

                if last > 1 {
                    feature.push(format!("4{}", next2_word)); // w[+2]
                    feature.push(format!("8{}{}", next_word, next2_word)); // w[+1]w[+2]
                    feature.push(format!("a{}{}", cur_word, next2_word)); // w[0]w[+2]
                }
            }

            if idx > 0 && last > 0 {
                // w[-1]w[0]w[+1]
                feature.push(format!("b{}{}{}", pre_word, cur_word, next_word));
            }

            features.push(feature);
        }
        features
    }

    pub fn parse_words_features_with_buffer<'a>(&self, words: &[&str], buffer: &'a mut Vec<u8>) -> Result<Vec<Vec<&'a str>>> {
        let word_null = "";
        let words_len = words.len();
        let mut features = Vec::with_capacity(words_len);

        let chars = words
            .iter()
            .map(|w| SmallVec::<[char; 4]>::from_iter(w.chars()))
            .collect_vec();

        for (idx, &cur_word) in words.iter().enumerate() {
            // 剩余字符数
            let last = words_len - idx - 1;
            let pre2_word = if idx > 1 { words[idx - 2] } else { word_null };
            let pre_word = if idx > 0 { words[idx - 1] } else { word_null };
            let next_word = if last > 0 { words[idx + 1] } else { word_null };
            let next2_word = if last > 1 { words[idx + 2] } else { word_null };

            // todo: 优化容量设置
            let mut feature = Vec::with_capacity(22);

            // w[0]
            buf_feature!(buffer, feature, "2{}", words[idx]);
            // ch[0,0]ch[0,n]
            buf_feature!(buffer, feature, "c{}{}", chars[idx][0], chars[idx][chars[idx].len() - 1]);
            // length
            buf_feature!(buffer, feature, "f{}", chars[idx].len());
            // prefix => ch[0,0]ch[0,0:1]ch[0,0:2]

            let prefix_id = &['c', 'd', 'e'];
            for (bias, prefix) in chars[idx]
                .iter()
                .take(3)
                .enumerate()
            {
                buf_feature!(buffer, feature, "{}{}", prefix_id[bias], prefix);
            };
            // suffix => ch[0,n-2:n],ch[0,n-1:n],ch[0,n]
            let suffix_id = &['f', 'g', 'h'];
            for (bias, suffix) in chars[idx]
                .iter()
                .rev()
                .take(3)
                .enumerate()
            {
                buf_feature!(buffer, feature, "{}{}", suffix_id[bias], suffix);
            };

            if idx > 0 {
                // w[-1]
                buf_feature!(buffer, feature, "1{}", pre_word);
                // w[-1]w[0]
                buf_feature!(buffer, feature, "6{}{}", pre_word, cur_word);
                // ch[-1,n]ch[0,0]
                buf_feature!(buffer, feature, "d{}{}", chars[idx - 1][chars[idx - 1].len() - 1], chars[idx][0]);

                if idx > 1 {
                    // w[-2]
                    buf_feature!(buffer, feature, "0{}", pre2_word);
                    // w[-2]w[-1]
                    buf_feature!(buffer, feature, "5{}{}", pre2_word, pre_word);
                    // w[-2]w[0]
                    buf_feature!(buffer, feature, "9{}{}", pre2_word, cur_word);
                }
            }

            if last > 0 {
                // w[+1]
                buf_feature!(buffer, feature, "3{}", next_word);
                // w[0]w[+1]
                buf_feature!(buffer, feature, "7{}{}", cur_word, next_word);
                // ch[0,-1]ch[1,0]
                buf_feature!(buffer, feature, "e{}{}", chars[idx][chars[idx].len() - 1], chars[idx + 1][0]);

                if last > 1 {
                    // w[+2]
                    buf_feature!(buffer, feature, "4{}", next2_word);
                    // w[+1]w[+2]
                    buf_feature!(buffer, feature, "8{}{}", next_word, next2_word);
                    // w[0]w[+2]
                    buf_feature!(buffer, feature, "a{}{}", cur_word, next2_word);
                }
            }

            if idx > 0 && last > 0 {
                // w[-1]w[0]w[+1]
                buf_feature!(buffer, feature, "b{}{}{}", pre_word, cur_word, next_word);
            }
            features.push(feature);
        }

        let mut start = 0usize;
        let mut result = Vec::with_capacity(features.len());
        for feature_end in features {
            let mut feature = Vec::with_capacity(feature_end.len());
            for end in feature_end {
                // Safety : all write are valid utf8
                feature.push(unsafe { std::str::from_utf8_unchecked(&buffer[start..end]) });
                start = end;
            }
            result.push(feature);
        }
        Ok(result)
    }
}

impl Definition for POSDefinition {
    type Fragment = dyn for<'any> GenericItem<'any, Item=()>;
    type Prediction = dyn for<'any> GenericItem<'any, Item=Vec<&'any str>>;
    type RawFeature = dyn for<'any> GenericItem<'any, Item=&'any [&'any str]>;

    fn labels(&self) -> Vec<String> {
        self.to_labels.clone()
    }

    fn label_num(&self) -> usize {
        self.to_labels.len()
    }

    fn label_to(&self, label: &str) -> usize {
        self.labels_to[label]
    }

    fn to_label(&self, index: usize) -> &str {
        &self.to_labels[index]
    }

    fn parse_features(&self, words: &&[&str]) -> ((), Vec<Vec<String>>) {
        let features = self.parse_words_features(words);
        ((), features)
    }

    fn parse_features_with_buffer<'a>(
        &self,
        words: &&[&str],
        buf: &'a mut Vec<u8>,
    ) -> Result<((), Vec<Vec<&'a str>>)> {
        let features = self.parse_words_features_with_buffer(words, buf)?;
        Ok(((), features))
    }

    #[cfg(feature = "parallel")]
    fn parse_gold_features<R: Read>(&self, reader: R) -> Vec<Sample> {
        let lines = BufReader::new(reader).lines();
        let lines = lines.flatten().filter(|s| !s.is_empty()).collect_vec();
        let mut result = Vec::with_capacity(lines.len());

        lines
            .par_iter()
            .map(|sentence| {
                let words_tags = sentence.split_whitespace().collect_vec();

                let mut words = Vec::with_capacity(words_tags.len());
                let mut labels = Vec::with_capacity(words_tags.len());
                for word_tag in words_tags {
                    let result = word_tag.rsplitn(2, '/');
                    let (label, word) = result.collect_tuple().expect("tag not found");
                    words.push(word);
                    labels.push(self.label_to(label));
                }
                let features = self.parse_words_features(&words);
                (features, labels)
            })
            .collect_into_vec(&mut result);

        result
    }

    #[cfg(not(feature = "parallel"))]
    fn parse_gold_features<R: Read>(&self, reader: R) -> Vec<Sample> {
        let lines = BufReader::new(reader).lines();
        let lines = lines.flatten().filter(|s| !s.is_empty()).collect_vec();

        lines
            .iter()
            .map(|sentence| {
                let words_tags = sentence.split_whitespace().collect_vec();

                let mut words = Vec::with_capacity(words_tags.len());
                let mut labels = Vec::with_capacity(words_tags.len());
                for word_tag in words_tags {
                    let result = word_tag.rsplitn(2, '/');
                    let (label, word) = result.collect_tuple().expect("tag not found");
                    words.push(word);
                    labels.push(self.label_to(label));
                }
                let features = self.parse_words_features(&words);
                (features, labels)
            })
            .collect_vec()
    }

    fn predict(
        &self,
        _: &<Self::RawFeature as GenericItem>::Item,
        _: &<Self::Fragment as GenericItem>::Item,
        predicts: &[usize],
    ) -> Vec<&str> {
        self.to_labels(predicts)
    }

    fn evaluate(&self, predicts: &[usize], labels: &[usize]) -> (usize, usize, usize) {
        self.evaluate_tags(predicts, labels)
    }
}


#[cfg(test)]
mod tests {
    use std::iter::zip;
    use super::POSDefinition as Define;
    use anyhow::Result;

    #[test]
    fn test_vec_buffer() -> Result<()> {
        let mut buffer = Vec::new();

        let sentence = vec!["桂林", "警备区", "从", "一九九○年", "以来", "，", "先后", "修建", "水电站", "十五", "座", "，", "整修", "水渠", "六千七百四十", "公里", "，", "兴修", "水利", "一千五百六十五", "处", "，", "修建", "机耕路", "一百二十六", "公里", "，", "修建", "人", "畜", "饮水", "工程", "二百六十五", "处", "，", "解决", "饮水", "人口", "六点五万", "人", "，", "使", "八万", "多", "壮", "、", "瑶", "、", "苗", "、", "侗", "、", "回", "等", "民族", "的", "群众", "脱", "了", "贫", "，", "占", "桂林", "地", "、", "市", "脱贫", "人口", "总数", "的", "百分之三十七点六", "。"];
        let define = Define::default();
        let no_buffer = define.parse_words_features(&sentence);
        let with_buffer = define.parse_words_features_with_buffer(&sentence, &mut buffer)?;

        for (a, b) in zip(no_buffer, with_buffer) {
            for (c, d) in zip(a, b) {
                assert_eq!(c, d);
            }
        }

        println!("{}/{}/{}", sentence.len(), buffer.len(), buffer.len() / sentence.len());

        Ok(())
    }
}

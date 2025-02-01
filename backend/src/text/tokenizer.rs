pub struct Tokenizer<'a> {
    content: &'a [char],
}

impl<'a> Tokenizer<'a> {
    pub fn new(content: &'a [char]) -> Self {
        Self { content }
    }

    fn next_token(&mut self) -> Option<&'a [char]> {
        loop {
            while !self.content.is_empty() && !self.content[0].is_ascii_graphic() {
                self.content = &self.content[1..];
            }

            if self.content.is_empty() {
                return None;
            }

            if self.content[0].is_ascii_graphic() {
                if self.content[0].is_ascii_punctuation() {
                    let token = &self.content[0..1];
                    self.content = &self.content[1..];
                    return Some(token);
                }
                let mut counter = 0;
                while counter < self.content.len() && self.content[counter].is_ascii_alphanumeric()
                {
                    counter += 1;
                }
                let token = &self.content[..counter];
                self.content = &self.content[counter..];
                return Some(token);
            }
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = &'a [char];

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

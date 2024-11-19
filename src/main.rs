use clap;
use clap::Parser;
use inline_colorization::*;
use rand::Rng;
use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
struct Cli {
    #[clap(short, long)]
    // specific word dictionary
    word_dictionary: Option<PathBuf>,
    #[clap(short, long, default_value_t = false)]
    // append to the word list (if word_dictionary is specified. otherwise this will replace the dictionary)
    append: bool,
    #[clap(long, default_value_t = false)]
    // hard mode (any yellow/green letters will need to be used on next guesses and green letters must stay where they are)
    hard: bool,
    #[clap(short, long, default_value_t = false)]
    // turn words.json into corrected words.txt
    format_json: bool,
}

struct Dictionary {
    words: Vec<String>,
}

impl Dictionary {
    #[allow(dead_code)]
    fn new() -> Self {
        Self { words: Vec::new() }
    }

    fn load(&mut self, path: PathBuf, append: bool) -> Result<(), Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let mut words: Vec<String> = serde_json::from_str(&contents)?;
        if append {
            self.words.append(&mut words);
        } else {
            self.words = words;
        }
        Ok(())
    }

    fn random(&self) -> String {
        let mut rng = rand::thread_rng();
        self.words[rng.gen_range(0..self.words.len())].clone()
    }

    fn have(&self, word: &str) -> bool {
        self.words.contains(&word.to_string())
    }
}

struct Game {
    dictionary: Dictionary,
    word: String,
    guesses: Vec<Vec<Guess>>,
    hard: bool,
    playing: bool,
    tries: u64,
    max_tries: u64,
    letter_counts: HashMap<char, i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Guess {
    Correct(char),
    Incorrect(char),
    Missed(char),
}

impl Guess {
    fn get_letter(&self) -> char {
        match self {
            Guess::Correct(letter) => *letter,
            Guess::Incorrect(letter) => *letter,
            Guess::Missed(letter) => *letter,
        }
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        let a: Vec<String> = serde_json::from_str(include_str!("../words.txt")).unwrap();
        Self { words: a }
    }
}

impl Game {
    fn new(dictionary: Dictionary, hard: bool) -> Self {
        Self {
            dictionary,
            word: "".to_string(),
            guesses: Vec::new(),
            hard,
            playing: false,
            tries: 1,
            max_tries: 5,
            letter_counts: HashMap::new(),
        }
    }

    fn play(&mut self) -> String {
        self.playing = true;
        let word;
        // this was a test case for the [`Game::determine_guess`] lmao
        if cfg!(debug_assertions) {
            word = "teats".to_string();
        } else {
            word = self.dictionary.random();
        }
        self.word = word.clone();
        self.letter_counts = word.chars().fold(HashMap::new(), |mut acc, letter| {
            *acc.entry(letter).or_insert(0) += 1;
            acc
        });
        word
    }

    fn determine_guess(&mut self, input: String) -> Result<Vec<Guess>, Errors> {
        if input.len() != self.word.len() {
            return Err(Errors::WordLengthNotEqualsToGuessWord);
        }
        if !self.dictionary.have(&input) {
            return Err(Errors::NoWordFound);
        }

        if self.tries > self.max_tries {
            return Err(Errors::MaximumTries(self.word.clone(), self.guesses.clone()));
        }
        
        if self.hard && self.guesses.len() != 0 { // check if in hard mode and if already guessed a word
            let last = self.guesses.last().unwrap();
            let correct_letter_poses = last.iter().enumerate().filter(|(_, c)| {
                match c {
                    Guess::Correct(_) => true,
                    _ => false,
                }
            }).map(|(i, c)| (i,c.get_letter())).collect::<Vec<(usize, char)>>();
            let missed_letters = last.iter().filter(|c| {
                match c {
                    Guess::Missed(_) => true,
                    _ => false,
                }
            }).map(|i| i.get_letter()).collect::<Vec<char>>();
            let collected_chars = input.chars().collect::<Vec<char>>();
            if !(correct_letter_poses.len() == 0) {
                for (i, c) in correct_letter_poses {
                    if let Some(ch) = collected_chars.get(i) {
                        if ch != &c {
                            return Err(Errors::InvalidWordInHardMode);
                        }
                    } else {
                        return Err(Errors::InvalidWordInHardMode);
                    }
                }
                for c in missed_letters {
                    if !(collected_chars.contains(&c)) {
                        return Err(Errors::InvalidWordInHardMode);
                    }
                }
            }
        }
    
        let mut guesses = vec![Guess::Incorrect('_'); input.len()];
        let mut correct_letters = 0;
        let mut cloned_word = self.letter_counts.clone();
    
        for (i, letter) in input.chars().enumerate() {
            if self.word.chars().nth(i).unwrap() == letter {
                guesses[i] = Guess::Correct(letter);
                correct_letters += 1;
                cloned_word.entry(letter).and_modify(|x| *x -= 1);
            }
        }
    
        for (i, letter) in input.chars().enumerate() {
            if guesses[i] == Guess::Incorrect('_') { // Only check remaining letters
                if self.word.contains(letter) && *cloned_word.entry(letter).or_insert(0) > 0 {
                    guesses[i] = Guess::Missed(letter);
                    cloned_word.entry(letter).and_modify(|x| *x -= 1);
                } else {
                    guesses[i] = Guess::Incorrect(letter);
                }
            }
        }
    
        self.guesses.push(guesses.clone());
        self.tries += 1;
    
        if correct_letters == self.word.len() {
            self.playing = false;
            let cloned_guesses = self.guesses.clone();
            let tries = self.tries;
            let max_tries = self.max_tries;
            self.reset();
            return Err(Errors::GameEndedWin(tries, max_tries, cloned_guesses));
        }
        Ok(guesses)
    }
    
    fn reset(&mut self) {
        self.tries = 1;
        self.guesses = Vec::new();
        self.letter_counts = HashMap::new();
        self.word = "".to_string();
    }
}

#[derive(Debug, Clone)]
enum Errors {
    NoWordFound,
    WordLengthNotEqualsToGuessWord,
    InvalidWordInHardMode,
    MaximumTries(String,Vec<Vec<Guess>>),
    GameEndedWin(u64, u64, Vec<Vec<Guess>>)
}

impl ToString for Errors {
    fn to_string(&self) -> String {
        match self {
            Errors::NoWordFound => "No word found".to_string(),
            Errors::WordLengthNotEqualsToGuessWord => {
                "Word length does not match guess word length".to_string()
            }
            Errors::InvalidWordInHardMode => "Invalid word in hard mode".to_string(),
            Errors::MaximumTries(_, _) => "Maximum tries reached".to_string(),
            Errors::GameEndedWin(_,_,_) => "Game ended with a win, please restart the game".to_string(),
        }
    }
}

fn main() {
    better_panic::Settings::new()
        .lineno_suffix(true)
        .verbosity(better_panic::Verbosity::Full)
        .install();

    let cli = Cli::parse();
    let mut game = Game::new(Dictionary::default(), cli.hard);
    if cli.format_json {
        let mut file = File::open("words.json").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let words: HashMap<String, u8> = serde_json::from_str(&contents).unwrap();
        let mut formatted = Vec::new();
        for (word, _) in words {
            if word.len() == 5 {
                formatted.push(word);
            }
        }
        serde_json::to_writer_pretty(File::create("words.txt").unwrap(), &formatted).unwrap();
        return;
    }

    if let Some(path) = cli.word_dictionary {
        game.dictionary
            .load(path, cli.append)
            .expect("Failed to load additonal word dictionary");
    }
    clearscreen::clear().ok();
    help();
    loop {
        let a = input(Some("Selection > "));
        if a.to_lowercase() == "help" {
            help();
        } else if a.to_lowercase() == "play" {
            play(&mut game);
        } else if a.to_lowercase() == "options" {
            options(&mut game);
        } else if a.to_lowercase() == "exit" {
            break;
        } else {
            println!("No options found");
        }
    }
}

fn options(game: &mut Game) {
    loop {
        clearscreen::clear().ok();
        println!("{color_cyan}R U S D L E{color_reset}");
        println!("Options:");
        println!(
            "1. Append/Replace to word list ({} words)",
            game.dictionary.words.len()
        );
        println!("2. Hard mode (yellow/green letters will need to be used on next guesses and green letters must stay where they are) ({})", if game.hard { "on" } else { "off" });
        println!("3. Tries ({} tries)", game.max_tries);
        println!("4. Exit");

        let ask = input(Some("Option > "));
        if ask.to_lowercase() == "1" {
            let append_or_replace =
                match input(Some("Append or Replace to word list? (True/False) > "))
                    .to_lowercase()
                    .as_str()
                {
                    "true" => true,
                    "false" => false,
                    _ => false,
                };
            loop {
                let option = input(Some("Append to word list (type q) (File path required) > "));
                if option.to_lowercase() == "q" {
                    break;
                }
                match game
                    .dictionary
                    .load(PathBuf::from(option), append_or_replace)
                {
                    Ok(()) => {
                        println!("{color_green}Loaded Successfully.{color_reset}");
                        break;
                    }
                    Err(e) => {
                        println!(
                            "{color_red}Failed to load.{color_reset} ({})",
                            e.to_string()
                        )
                    }
                }
            }
        } else if ask.to_lowercase() == "2" {
            game.hard = !game.hard;
            println!("Hard mode is now {}", if game.hard { "on" } else { "off" });
        } else if ask.to_lowercase() == "3" {
            let tries = match input(Some("Tries (type q) > ")).to_lowercase().as_str() {
                "q" => 5,
                _ => input(Some("Tries (type q) > ")).parse::<u64>().unwrap(),
            };
            game.max_tries = tries;
            println!("Tries is now {}", tries);
        } else {
            break;
        }
    }
}

fn show_text(game: &Game) {
    if cfg!(debug_assertions) {
        println!("{color_cyan}R U S D L E (Word is {}){color_reset}", game.word);
    } else {
        println!("{color_cyan}R U S D L E (Word is {} characters long) {color_reset}", game.word.len());
    }
    println!(
        "{}",
        game.guesses
            .iter()
            .map(|i| {
                i.iter()
                    .map(|j| match j {
                        Guess::Correct(letter) => {
                            format!("{bg_green}{color_black}{}{color_reset}{bg_reset}", letter)
                        }
                        Guess::Incorrect(letter) => {
                            format!("{bg_red}{color_black}{}{color_reset}{bg_reset}", letter)
                        }
                        Guess::Missed(letter) => {
                            format!("{bg_yellow}{color_black}{}{color_reset}{bg_reset}", letter)
                        }
                    })
                    .collect()
            })
            .collect::<Vec<Vec<String>>>()
            .iter()
            .map(|i| { i.join(" ") })
            .collect::<Vec<String>>()
            .join("\n\n")
    );
}

fn play(game: &mut Game) {
    game.play();
    clearscreen::clear().ok();
    loop {
        if cfg!(debug_assertions) {
            println!("{color_cyan}R U S D L E (Word is {}) (Tries: {}/{} Tries{}){color_reset}", game.word, game.tries, game.max_tries, if game.hard { " (Hard Mode)" } else { "" });
        } else {
            println!("{color_cyan}R U S D L E (Word is {} characters long) (Tries: {}/{} Tries{}){color_reset}", game.word.len(), game.tries, game.max_tries, if game.hard { " (Hard Mode)" } else { "" });
        }
        let input = input(Some("Guess > "));
        let guesses = game.determine_guess(input);
        match guesses {
            Ok(_) => {
                clearscreen::clear().ok();
                show_text(game);
                println!();
            }
            Err(e) => match e {
                Errors::MaximumTries(word, guesses) => {
                    show_text(game);
                    println!("{color_yellow}Maximum tries reached, exiting...{color_reset}");
                    println!("{color_red}The word was {}{color_reset}", word);
                    println!("{color_green}Your accuracy is {}%{color_reset}", calculate_guess_accuracy(guesses) * 100.0);
                    break;
                },
                Errors::GameEndedWin(tries, max_tries, guesses) => {
                    println!("{color_green}You win!{color_reset}");
                    println!("{bg_black}{color_bright_white} Took {}/{} tries.{color_reset}{bg_reset}", tries - 1, max_tries);
                    println!("{color_green}Your accuracy is {}%{color_reset}", calculate_guess_accuracy(guesses.clone()) * 100.0);
                    break;
                }
                _ => {
                    println!("{color_red}ERROR: {}{color_reset}", e.to_string());
                }
            },
        }
    }
}

fn calculate_guess_accuracy(guesses: Vec<Vec<Guess>>) -> f64 {
    let mut points = 0.0;
    let maximum_possible_point = guesses.first().unwrap().len() * 2; // 2 points per correct letter
    for guess in &guesses {
        for g in guess {
            if let Guess::Correct(_) = g { points += 2.0; }
            else if let Guess::Missed(_) = g { points += 1.0; }
            else {
                points -= 0.5;
            }
        }
    }
    (points as f64 / guesses.len() as f64) / maximum_possible_point as f64
}

fn help() {
    println!("{color_cyan}RUSDLE{color_reset}");
    println!(
        "Welcome to {color_cyan}PORDLE{color_reset}! Please run the program with {bg_black}{color_bright_white}-h{color_reset}{bg_reset} for additional flags like hard mode! (or you can manually configure this inside the game)" 
    );
    println!(
        "Any configurable options can be changed with {bg_black}{color_bright_white}options{color_reset}{bg_reset} as the input!"
    );
    println!(
        "And when you are ready to play, type {bg_black}{color_bright_white}play{color_reset}{bg_reset}!"
    );
}

fn input(ask: Option<&str>) -> String {
    print!("{}", ask.unwrap_or(""));
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim_end().to_string()
}

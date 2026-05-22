use crate::state::{State, Value};

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    StringLit(String),
    Bool(bool),
    Ident(String),
    ResultVar,
    Op(Op),
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
enum Expr {
    Literal(Value),
    Var(String),
    ResultVar,
    BinOp(Box<Expr>, Op, Box<Expr>),
    UnaryNot(Box<Expr>),
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => { i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '\'' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::StringLit(s));
                if i < chars.len() { i += 1; }
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Op(Op::Eq));
                i += 2;
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Op(Op::Ne));
                i += 2;
            }
            '<' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Op(Op::Le));
                i += 2;
            }
            '>' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::Op(Op::Ge));
                i += 2;
            }
            '<' => { tokens.push(Token::Op(Op::Lt)); i += 1; }
            '>' => { tokens.push(Token::Op(Op::Gt)); i += 1; }
            '+' => { tokens.push(Token::Op(Op::Add)); i += 1; }
            '-' => { tokens.push(Token::Op(Op::Sub)); i += 1; }
            '*' => { tokens.push(Token::Op(Op::Mul)); i += 1; }
            '/' => { tokens.push(Token::Op(Op::Div)); i += 1; }
            '$' => {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                if name == "result" {
                    tokens.push(Token::ResultVar);
                } else {
                    tokens.push(Token::Ident(format!("${}", name)));
                }
            }
            c if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) => {
                let start = i;
                let mut has_dot = false;
                while i < chars.len() && (chars[i].is_ascii_digit() || (chars[i] == '.' && !has_dot)) {
                    if chars[i] == '.' { has_dot = true; }
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                let num: f64 = num_str.parse().unwrap_or(0.0);
                tokens.push(Token::Number(num));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.as_str() {
                    "true" => tokens.push(Token::Bool(true)),
                    "false" => tokens.push(Token::Bool(false)),
                    "and" => tokens.push(Token::Op(Op::And)),
                    "or" => tokens.push(Token::Op(Op::Or)),
                    "not" => tokens.push(Token::Op(Op::Not)),
                    _ => tokens.push(Token::Ident(word)),
                }
            }
            _ => { i += 1; }
        }
    }
    tokens
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while matches!(self.peek(), Some(Token::Op(Op::Or))) {
            self.advance();
            let right = self.parse_and();
            left = Expr::BinOp(Box::new(left), Op::Or, Box::new(right));
        }
        left
    }

    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_not();
        while matches!(self.peek(), Some(Token::Op(Op::And))) {
            self.advance();
            let right = self.parse_not();
            left = Expr::BinOp(Box::new(left), Op::And, Box::new(right));
        }
        left
    }

    fn parse_not(&mut self) -> Expr {
        if matches!(self.peek(), Some(Token::Op(Op::Not))) {
            self.advance();
            let expr = self.parse_comparison();
            return Expr::UnaryNot(Box::new(expr));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Expr {
        let left = self.parse_additive();
        if let Some(Token::Op(op @ (Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge))) = self.peek().cloned() {
            self.advance();
            let right = self.parse_additive();
            return Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_additive(&mut self) -> Expr {
        let mut left = self.parse_multiplicative();
        while let Some(Token::Op(op @ (Op::Add | Op::Sub))) = self.peek().cloned() {
            self.advance();
            let right = self.parse_multiplicative();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_multiplicative(&mut self) -> Expr {
        let mut left = self.parse_primary();
        while let Some(Token::Op(op @ (Op::Mul | Op::Div))) = self.peek().cloned() {
            self.advance();
            let right = self.parse_primary();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_primary(&mut self) -> Expr {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                if n == (n as i64) as f64 {
                    Expr::Literal(Value::Int(n as i64))
                } else {
                    Expr::Literal(Value::Float(n))
                }
            }
            Some(Token::StringLit(s)) => {
                self.advance();
                Expr::Literal(Value::Str(s))
            }
            Some(Token::Bool(b)) => {
                self.advance();
                Expr::Literal(Value::Bool(b))
            }
            Some(Token::Ident(name)) => {
                self.advance();
                Expr::Var(name)
            }
            Some(Token::ResultVar) => {
                self.advance();
                Expr::ResultVar
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr();
                if matches!(self.peek(), Some(Token::RParen)) {
                    self.advance();
                }
                expr
            }
            _ => {
                self.advance();
                Expr::Literal(Value::Bool(false))
            }
        }
    }
}

fn eval_expr(expr: &Expr, state: &State) -> Value {
    match expr {
        Expr::Literal(v) => v.clone(),
        Expr::Var(name) => {
            state.get_value(name).unwrap_or(Value::Bool(false))
        }
        Expr::ResultVar => {
            state.get_value("$result").unwrap_or(Value::Str(String::new()))
        }
        Expr::UnaryNot(inner) => {
            let val = eval_expr(inner, state);
            Value::Bool(!val.is_truthy())
        }
        Expr::BinOp(left, op, right) => {
            let lv = eval_expr(left, state);
            let rv = eval_expr(right, state);
            match op {
                Op::And => Value::Bool(lv.is_truthy() && rv.is_truthy()),
                Op::Or => Value::Bool(lv.is_truthy() || rv.is_truthy()),
                Op::Eq => Value::Bool(values_equal(&lv, &rv)),
                Op::Ne => Value::Bool(!values_equal(&lv, &rv)),
                Op::Lt => Value::Bool(compare_values(&lv, &rv) == Some(std::cmp::Ordering::Less)),
                Op::Gt => Value::Bool(compare_values(&lv, &rv) == Some(std::cmp::Ordering::Greater)),
                Op::Le => Value::Bool(matches!(compare_values(&lv, &rv), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal))),
                Op::Ge => Value::Bool(matches!(compare_values(&lv, &rv), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal))),
                Op::Add => arithmetic(&lv, &rv, |a, b| a + b),
                Op::Sub => arithmetic(&lv, &rv, |a, b| a - b),
                Op::Mul => arithmetic(&lv, &rv, |a, b| a * b),
                Op::Div => arithmetic(&lv, &rv, |a, b| if b != 0.0 { a / b } else { 0.0 }),
                Op::Not => unreachable!(),
            }
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
        (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
        (Value::Str(a), Value::Str(b)) => a == b,
        _ => false,
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::Str(a), Value::Str(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

fn arithmetic(a: &Value, b: &Value, f: fn(f64, f64) -> f64) -> Value {
    let av = to_number(a);
    let bv = to_number(b);
    let result = f(av, bv);
    if result == (result as i64) as f64 && matches!((a, b), (Value::Int(_), Value::Int(_))) {
        Value::Int(result as i64)
    } else {
        Value::Float(result)
    }
}

fn to_number(v: &Value) -> f64 {
    match v {
        Value::Int(n) => *n as f64,
        Value::Float(f) => *f,
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        Value::Str(s) => s.parse().unwrap_or(0.0),
    }
}

pub fn evaluate_when(expr_str: &str, state: &State) -> bool {
    let tokens = tokenize(expr_str);
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_expr();
    let result = eval_expr(&ast, state);
    result.is_truthy()
}

pub fn evaluate_expr(expr_str: &str, state: &State) -> Value {
    let tokens = tokenize(expr_str);
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_expr();
    eval_expr(&ast, state)
}

pub fn evaluate_do_value(_key: &str, raw_value: &serde_json::Value, state: &State) -> Value {
    match raw_value {
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        serde_json::Value::String(s) => {
            if looks_like_expression(s, state) {
                evaluate_expr(s, state)
            } else {
                Value::Str(s.clone())
            }
        }
        _ => Value::Str(raw_value.to_string()),
    }
}

fn looks_like_expression(s: &str, state: &State) -> bool {
    let operators = ["+", "-", "*", "/", "==", "!=", "<", ">", "<=", ">=", "and", "or", "not"];
    for op in &operators {
        if s.contains(op) {
            return true;
        }
    }
    let tokens = tokenize(s);
    if tokens.len() == 1
        && let Some(Token::Ident(name)) = tokens.first() {
            return state.has_var(name);
        }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_state() -> State {
        let mut vars = HashMap::new();
        vars.insert("heard_elmar".into(), Value::Bool(false));
        vars.insert("heard_veri".into(), Value::Bool(true));
        vars.insert("hearing_count".into(), Value::Int(2));
        vars.insert("accused".into(), Value::Str("".into()));
        State::new(vars)
    }

    #[test]
    fn test_not_bool() {
        let state = make_state();
        assert!(evaluate_when("not heard_elmar", &state));
        assert!(!evaluate_when("not heard_veri", &state));
    }

    #[test]
    fn test_compare_int() {
        let state = make_state();
        assert!(evaluate_when("hearing_count >= 1", &state));
        assert!(evaluate_when("hearing_count >= 2", &state));
        assert!(!evaluate_when("hearing_count >= 3", &state));
    }

    #[test]
    fn test_compare_string() {
        let mut state = make_state();
        state.set("accused", Value::Str("til".into()));
        assert!(evaluate_when("accused == 'til'", &state));
        assert!(!evaluate_when("accused == 'elmar'", &state));
    }

    #[test]
    fn test_logical_and_or() {
        let mut state = make_state();
        state.set("heard_elmar", Value::Bool(true));
        assert!(evaluate_when("heard_elmar and heard_veri", &state));
        assert!(evaluate_when("heard_elmar or heard_veri", &state));
        state.set("heard_elmar", Value::Bool(false));
        assert!(!evaluate_when("heard_elmar and heard_veri", &state));
        assert!(evaluate_when("heard_elmar or heard_veri", &state));
    }

    #[test]
    fn test_arithmetic() {
        let state = make_state();
        let result = evaluate_expr("hearing_count + 1", &state);
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_result_variable() {
        let mut state = make_state();
        state.set_result(Some("elmar".into()));
        assert!(evaluate_when("$result == 'elmar'", &state));
        assert!(!evaluate_when("$result == 'til'", &state));
    }

    #[test]
    fn test_do_expression() {
        let state = make_state();
        let val = evaluate_do_value("hearing_count", &serde_json::json!("hearing_count + 1"), &state);
        assert_eq!(val, Value::Int(3));

        let val = evaluate_do_value("heard_elmar", &serde_json::json!(true), &state);
        assert_eq!(val, Value::Bool(true));

        let val = evaluate_do_value("accused", &serde_json::json!("til"), &state);
        assert_eq!(val, Value::Str("til".into()));
    }

    #[test]
    fn test_compound_expression() {
        let mut state = make_state();
        // accused is "" → accused == '' is true → not true = false
        assert!(!evaluate_when("hearing_count >= 1 and not accused == ''", &state));
        // set accused to something → not accused == '' → true
        state.set("accused", Value::Str("til".into()));
        assert!(evaluate_when("hearing_count >= 1 and not accused == ''", &state));
    }
}

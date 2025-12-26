pub fn levenshtein(a: &str, b: &str) -> usize {
    let len_a = a.chars().count();
    let len_b = b.chars().count();
    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }

    let mut dp = vec![vec![0; len_b + 1]; len_a + 1];

    for i in 0..=len_a {
        dp[i][0] = i;
    }
    for j in 0..=len_b {
        dp[0][j] = j;
    }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = std::cmp::min(
                dp[i][j + 1] + 1,
                std::cmp::min(dp[i + 1][j] + 1, dp[i][j] + cost),
            );
        }
    }
    dp[len_a][len_b]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("book", "back"), 2);
        assert_eq!(levenshtein("kubernetes", "kubernetes"), 0);
        assert_eq!(levenshtein("kubernetes", "kbernetes"), 1); // missing u
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }
}

pub fn format_role_message(
    conclusion: &str,
    evidence: &str,
    uncertainty: &str,
    next_step: &str,
) -> String {
    format!(
        "结论\n{}\n\n依据\n{}\n\n不确定项\n{}\n\n下一步\n{}",
        conclusion.trim(),
        evidence.trim(),
        uncertainty.trim(),
        next_step.trim()
    )
}

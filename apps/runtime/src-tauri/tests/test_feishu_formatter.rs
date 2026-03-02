use runtime_lib::im::feishu_formatter::format_role_message;

#[test]
fn formatter_outputs_required_sections() {
    let msg = format_role_message(
        "可以承接该项目",
        "已有两次同类交付案例，技术栈兼容",
        "客户现有系统接口文档不完整",
        "安排一次技术澄清会并补齐接口清单",
    );

    assert!(msg.contains("结论"));
    assert!(msg.contains("依据"));
    assert!(msg.contains("不确定项"));
    assert!(msg.contains("下一步"));
}


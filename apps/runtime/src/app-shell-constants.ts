export const MODEL_SETUP_STEPS: Array<{ title: string; description: string }> = [
  {
    title: "选择一个服务商模板",
    description: "优先选你已经有 API Key 的平台，系统会自动带出推荐参数。",
  },
  {
    title: "填入 API Key",
    description: "首次接入只需要这一步，其他字段后续都能在设置里细调。",
  },
  {
    title: "补齐搜索引擎",
    description: "模型保存成功后继续配置搜索，让智能体开箱即可联网检索。",
  },
];

export const MODEL_SETUP_OUTCOMES = ["创建会话", "执行技能", "驱动智能体员工协作"];

export const SHOW_DEV_MODEL_SETUP_TOOLS = import.meta.env.DEV || import.meta.env.MODE === "test";

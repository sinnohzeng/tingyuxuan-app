/**
 * 引导流程 — Sprint 5 实现
 * 当前为占位组件，后续填充介绍滑页 + API 配置向导 + 权限检查。
 */
import { useNavigate } from "react-router-dom";

export default function OnboardingFlow() {
  const navigate = useNavigate();

  return (
    <div>
      <h2>欢迎使用听语轩</h2>
      <p>引导流程占位 — Sprint 5 实现</p>
      <button onClick={() => navigate("/main")}>跳过，进入首页</button>
    </div>
  );
}

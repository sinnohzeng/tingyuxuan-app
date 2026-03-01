import { Component, type ErrorInfo, type ReactNode } from "react";
import { createLogger } from "../lib/logger";

const log = createLogger("ErrorBoundary");

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * Catches rendering errors in child components and displays a fallback UI
 * instead of a white screen.
 */
export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    log.error("Render error caught", { error: error.message, componentStack: info.componentStack });
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center h-screen p-6 bg-gray-900 text-gray-200">
          <h2 className="text-lg font-semibold mb-2">出了点问题</h2>
          <p className="text-sm text-gray-400 mb-4 text-center max-w-sm">
            {this.state.error?.message || "应用遇到了意外错误"}
          </p>
          <button
            onClick={this.handleRetry}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded-lg transition-colors"
          >
            重试
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

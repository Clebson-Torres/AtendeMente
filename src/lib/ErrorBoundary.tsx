import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen items-center justify-center bg-red-50 p-8">
          <div className="bg-white rounded-xl shadow-md p-6 max-w-lg">
            <h1 className="text-xl font-bold text-red-600 mb-2">
              Erro na aplicação
            </h1>
            <pre className="text-sm text-red-500 whitespace-pre-wrap">
              {this.state.error.message}
              {"\n\n"}
              {this.state.error.stack}
            </pre>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

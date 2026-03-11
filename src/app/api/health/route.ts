import { jsonWithRequestId } from "@/lib/observability/request";

export function GET(request: Request) {
  return jsonWithRequestId(
    request,
    {
      status: "ok",
      timestamp: new Date().toISOString(),
      environment: process.env.NODE_ENV ?? "development",
      version: process.env.VERCEL_GIT_COMMIT_SHA ?? "local",
    },
    { status: 200 },
  );
}

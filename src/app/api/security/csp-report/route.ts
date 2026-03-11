import { logWarn } from "@/lib/observability/logger";
import { getRequestId } from "@/lib/observability/request";

async function readCspPayload(request: Request) {
  const rawBody = await request.text();

  if (!rawBody) {
    return null;
  }

  try {
    return JSON.parse(rawBody);
  } catch {
    return { rawBody };
  }
}

export async function POST(request: Request) {
  const payload = await readCspPayload(request);
  const report =
    payload && typeof payload === "object" && !Array.isArray(payload)
      ? (payload["csp-report" as keyof typeof payload] ??
          payload["body" as keyof typeof payload] ??
          payload)
      : payload;

  const normalizedReport =
    report && typeof report === "object" && !Array.isArray(report)
      ? {
          documentUri: report["document-uri" as keyof typeof report] ?? null,
          violatedDirective: report["violated-directive" as keyof typeof report] ?? null,
          effectiveDirective: report["effective-directive" as keyof typeof report] ?? null,
          blockedUri: report["blocked-uri" as keyof typeof report] ?? null,
          sourceFile: report["source-file" as keyof typeof report] ?? null,
          lineNumber: report["line-number" as keyof typeof report] ?? null,
          disposition: report["disposition" as keyof typeof report] ?? null,
          originalPolicy: report["original-policy" as keyof typeof report] ?? null,
        }
      : { rawPayload: report };

  logWarn("security.csp_report", {
    requestId: getRequestId(request),
    userAgent: request.headers.get("user-agent"),
    ...normalizedReport,
    payload,
  });

  return new Response(null, { status: 204 });
}

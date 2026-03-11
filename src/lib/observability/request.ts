import { NextResponse } from "next/server";

export function getRequestId(request: Request) {
  return request.headers.get("x-request-id") ?? null;
}

export function getForwardedIp(request: Request) {
  return request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ?? null;
}

export function buildRequestLogContext(
  request: Request,
  route: string,
  extras: Record<string, unknown> = {},
) {
  return {
    requestId: getRequestId(request),
    route,
    method: request.method,
    ipAddress: getForwardedIp(request),
    ...extras,
  };
}

export function jsonWithRequestId(
  request: Request,
  body: Parameters<typeof NextResponse.json>[0],
  init?: ResponseInit,
) {
  const response = NextResponse.json(body, init);
  const requestId = getRequestId(request);

  if (requestId) {
    response.headers.set("x-request-id", requestId);
  }

  return response;
}

import { describe, it, vi, expect, beforeEach } from "vitest";

const API = `${window.location.origin}/api`;

type FetchInput = { url: string; method: string };
let fetchCalls: FetchInput[] = [];

function mockFetchOnce(status: number, body: unknown) {
  globalThis.fetch = vi.fn().mockImplementationOnce(() =>
    Promise.resolve({
      ok: status >= 200 && status < 300,
      json: () => Promise.resolve(body),
    } as Response)
  );
}

describe("auth login flow", () => {
  beforeEach(async () => {
    vi.resetModules();
    fetchCalls = [];
    globalThis.fetch = vi.fn().mockImplementation((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });
  });

  it("register calls notify with onboarding_completed = false", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: {
              token: "tok-abc",
              user_id: "uid-123",
              email: "novo@test.com",
              full_name: "Novo User",
              recovery_secret: "ABCD-EFGH-IJKL-MNOP",
              onboarding_completed: false,
            },
          }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.register("novo@test.com", "senha12345", "Novo User");

    expect(notifiedUser).not.toBeNull();
    expect(notifiedUser!.uid).toBe("uid-123");
    expect(notifiedUser!.onboarding_completed).toBe(false);
    expect(auth.getPendingRecoverySecret()).toBe("ABCD-EFGH-IJKL-MNOP");
  });

  it("login returns onboarding_completed from server", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: {
              token: "tok-456",
              user_id: "uid-456",
              email: "existente@test.com",
              full_name: "Existing User",
              onboarding_completed: true,
            },
          }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.login("existente@test.com", "senha12345");

    expect(notifiedUser).not.toBeNull();
    expect(notifiedUser!.uid).toBe("uid-456");
    expect(notifiedUser!.onboarding_completed).toBe(true);
  });

  it("completeOnboarding updates local state even when PATCH fails", async () => {
    // First call = register succeeds
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: {
              token: "tok-789",
              user_id: "uid-789",
              email: "onboard@test.com",
              full_name: "Onboard User",
              recovery_secret: "1234-5678-90AB-CDEF",
              onboarding_completed: false,
            },
          }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.register("onboard@test.com", "senha12345", "Onboard User");
    expect(notifiedUser!.onboarding_completed).toBe(false);

    // Second call = PATCH fails (network error)
    globalThis.fetch = vi.fn().mockRejectedValueOnce(new Error("Network error"));

    await auth.completeOnboarding();

    // State should be updated locally despite API failure
    expect(notifiedUser!.onboarding_completed).toBe(true);
  });

  it("completeOnboarding calls PATCH and updates state on success", async () => {
    // Register
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: {
              token: "tok-101",
              user_id: "uid-101",
              email: "patch@test.com",
              full_name: "Patch User",
              recovery_secret: "FEDC-BA09-8765-4321",
              onboarding_completed: false,
            },
          }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.register("patch@test.com", "senha12345", "Patch User");
    expect(notifiedUser!.onboarding_completed).toBe(false);

    // PATCH succeeds
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    await auth.completeOnboarding();

    expect(notifiedUser!.onboarding_completed).toBe(true);

    // Verify PATCH was called
    const patchCall = fetchCalls.find(
      (c) => c.url === `${API}/auth/onboarding` && c.method === "PATCH"
    );
    expect(patchCall).toBeDefined();
  });

  it("completeFromStoredToken sets onboarding_completed from /auth/me", async () => {
    // Register
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: {
              token: "tok-202",
              user_id: "uid-202",
              email: "me@test.com",
              full_name: "Me User",
              recovery_secret: "AAAA-BBBB-CCCC-DDDD",
              onboarding_completed: false,
            },
          }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");
    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.register("me@test.com", "senha12345", "Me User");

    // Mock /auth/me to return onboarding_completed = true
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () =>
          Promise.resolve({
            success: true,
            data: { user_id: "uid-202", email: "me@test.com", onboarding_completed: true },
          }),
      } as Response)
    );

    await auth.completeFromStoredToken();

    expect(notifiedUser!.onboarding_completed).toBe(true);
  });

  it("recoverPassword works with email and recovery_secret", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: { reset_token: "reset-token-123" },
        }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");

    const token = await auth.recoverPassword({
      email: "user@test.com",
      recovery_secret: "ABCD-EFGH-IJKL-MNOP",
    });

    expect(token).toBe("reset-token-123");
    const call = fetchCalls.find((c) => c.url === `${API}/auth/recover`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("POST");
  });

  it("recoverPassword works with user_id and recovery_secret", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: { reset_token: "reset-token-456" },
        }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");

    const token = await auth.recoverPassword({
      user_id: "uid-123",
      recovery_secret: "1234-5678-90AB-CDEF",
    });

    expect(token).toBe("reset-token-456");
    const call = fetchCalls.find((c) => c.url === `${API}/auth/recover`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("POST");
  });

  it("logout clears token and notifies null", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = "still-set";
    auth.onAuthChange((u) => { notifiedUser = u; });

    // Simulate being logged in by first registering
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: {
            token: "tok-logout",
            user_id: "uid-logout",
            email: "logout@test.com",
            full_name: "Logout User",
            recovery_secret: "AAAA-BBBB-CCCC-DDDD",
            onboarding_completed: true,
          },
        }),
      } as Response)
    );
    await auth.register("logout@test.com", "senha12345", "Logout User");

    // Reset notifiedUser for logout test
    notifiedUser = "still-set";

    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    await auth.logout();

    expect(notifiedUser).toBeNull();
    const call = fetchCalls.find((c) => c.url === `${API}/auth/logout`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("GET");
  });

  it("logout without token skips API request", async () => {
    const auth = await import("../src/lib/auth");

    let notifiedUser: any = "still-set";
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.logout();

    expect(notifiedUser).toBeNull();
    const call = fetchCalls.find((c) => c.url === `${API}/auth/logout`);
    expect(call).toBeUndefined();
  });

  it("logout ignores API error and clears locally", async () => {
    globalThis.fetch = vi.fn().mockRejectedValueOnce(new Error("Network error"));

    const auth = await import("../src/lib/auth");

    let notifiedUser: any = "still-set";
    auth.onAuthChange((u) => { notifiedUser = u; });

    // Simulate being logged in
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: {
            token: "tok-err",
            user_id: "uid-err",
            email: "err@test.com",
            full_name: "Err User",
            recovery_secret: "EEEE-FFFF-GGGG-HHHH",
            onboarding_completed: true,
          },
        }),
      } as Response)
    );
    await auth.register("err@test.com", "senha12345", "Err User");

    notifiedUser = "still-set";

    await auth.logout();
    expect(notifiedUser).toBeNull();
  });

  it("lock calls /auth/lock", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");
    await auth.lock();

    const call = fetchCalls.find((c) => c.url === `${API}/auth/lock`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("GET");
  });

  it("unlock calls /auth/unlock with password", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");
    await auth.unlock("minha-senha");

    const call = fetchCalls.find((c) => c.url === `${API}/auth/unlock`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("POST");
  });

  it("resetPassword calls /auth/reset-password with token and new_password", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ success: true, data: null }),
      } as Response);
    });

    const auth = await import("../src/lib/auth");
    await auth.resetPassword("reset-token-xyz", "nova-senha-12345");

    const call = fetchCalls.find((c) => c.url === `${API}/auth/reset-password`);
    expect(call).toBeDefined();
    expect(call!.method).toBe("POST");
  });

  it("restoreSession calls /auth/me when token exists", async () => {
    globalThis.fetch = vi.fn().mockImplementationOnce(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: {
            token: "tok-restore",
            user_id: "uid-restore",
            email: "restore@test.com",
            full_name: "Restore User",
            recovery_secret: "RRRR-EEEE-SSSS-TTTT",
            onboarding_completed: true,
          },
        }),
      } as Response)
    );

    const auth = await import("../src/lib/auth");
    await auth.register("restore@test.com", "senha12345", "Restore User");

    let notifiedUser: any = null;
    auth.onAuthChange((u) => { notifiedUser = u; });

    // Now restoreSession — token exists, should call /auth/me
    globalThis.fetch = vi.fn().mockImplementationOnce((url: string, opts?: RequestInit) => {
      fetchCalls.push({ url: url as string, method: opts?.method || "GET" });
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve({
          success: true,
          data: { user_id: "uid-restore", email: "restore@test.com", onboarding_completed: true },
        }),
      } as Response);
    });

    fetchCalls = [];
    await auth.restoreSession();

    const meCall = fetchCalls.find((c) => c.url === `${API}/auth/me`);
    expect(meCall).toBeDefined();
    expect(notifiedUser?.onboarding_completed).toBe(true);
  });

  it("restoreSession notifies null when no token", async () => {
    const auth = await import("../src/lib/auth");

    let notifiedUser: any = "still-set";
    auth.onAuthChange((u) => { notifiedUser = u; });

    await auth.restoreSession();

    expect(notifiedUser).toBeNull();
  });
});

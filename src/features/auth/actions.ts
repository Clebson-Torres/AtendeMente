"use server";

import { redirect } from "next/navigation";
import { createAdminSupabaseClient } from "@/lib/supabase/admin";
import { writeAuditLog } from "@/lib/audit/log";
import { getCurrentUser } from "@/lib/auth/session";
import { getOptionalEnv } from "@/lib/env";
import { AppError } from "@/lib/errors/app-error";
import { logError, logInfo, logWarn } from "@/lib/observability/logger";
import { enforceRateLimit } from "@/lib/security/rate-limit";
import { createServerSupabaseClient } from "@/lib/supabase/server";
import { forgotPasswordSchema, loginSchema } from "@/features/auth/schemas";
import type { ActionResponse } from "@/types/domain";

export async function signInAction(
  input: unknown,
  redirectTo = "/dashboard",
): Promise<ActionResponse<{ redirectTo: string }>> {
  try {
    const parsed = loginSchema.safeParse(input);

    if (!parsed.success) {
      return {
        success: false,
        message: "Verifique os campos obrigatorios.",
        errors: parsed.error.flatten().fieldErrors,
      };
    }

    await enforceRateLimit({
      scope: "auth:login",
      identifier: `${parsed.data.email.toLowerCase()}`,
      limit: 5,
      windowMs: 10 * 60 * 1000,
    });

    const supabase = await createServerSupabaseClient();
    const { data, error } = await supabase.auth.signInWithPassword(parsed.data);

    if (error || !data.user) {
      logWarn("auth.sign_in.invalid_credentials", {
        email: parsed.data.email.toLowerCase(),
      });
      return {
        success: false,
        message: "Nao foi possivel entrar. Confira email e senha.",
      };
    }

    await writeAuditLog({
      userId: data.user.id,
      action: "login",
      entityType: "auth",
      entityId: data.user.id,
    });

    logInfo("auth.sign_in.success", {
      userId: data.user.id,
    });

    return {
      success: true,
      message: "Login realizado com sucesso.",
      data: { redirectTo },
    };
  } catch (error) {
    if (error instanceof AppError) {
      logWarn("auth.sign_in.app_error", {
        statusCode: error.statusCode,
        errorCode: error.code,
      });
      return {
        success: false,
        message: error.message,
      };
    }

    logError("auth.sign_in.failed", error);
    return {
      success: false,
      message: "Nao foi possivel entrar agora. Tente novamente em instantes.",
    };
  }
}

export async function sendPasswordResetAction(input: unknown): Promise<ActionResponse> {
  try {
    const parsed = forgotPasswordSchema.safeParse(input);

    if (!parsed.success) {
      return {
        success: false,
        message: "Informe um email valido.",
        errors: parsed.error.flatten().fieldErrors,
      };
    }

    await enforceRateLimit({
      scope: "auth:password-reset",
      identifier: parsed.data.email.toLowerCase(),
      limit: 3,
      windowMs: 15 * 60 * 1000,
    });

    const supabase = await createServerSupabaseClient();
    const appUrl = getOptionalEnv("NEXT_PUBLIC_APP_URL") ?? "http://localhost:3000";
    await supabase.auth.resetPasswordForEmail(parsed.data.email, {
      redirectTo: `${appUrl}/reset-password`,
    });

    logInfo("auth.password_reset.requested", {
      email: parsed.data.email.toLowerCase(),
    });

    return {
      success: true,
      message: "Se o email existir, enviaremos um link para redefinir a senha.",
    };
  } catch (error) {
    if (error instanceof AppError) {
      logWarn("auth.password_reset.app_error", {
        statusCode: error.statusCode,
        errorCode: error.code,
      });
      return {
        success: false,
        message: error.message,
      };
    }

    logError("auth.password_reset.failed", error);
    return {
      success: false,
      message: "Nao foi possivel solicitar a redefinicao agora.",
    };
  }
}

export async function sendInviteAction(
  input: { email: string; fullName?: string | null },
): Promise<ActionResponse> {
  try {
    const email = input.email.trim().toLowerCase();

    if (!email) {
      return {
        success: false,
        message: "Informe um email valido para convite.",
      };
    }

    await enforceRateLimit({
      scope: "auth:invite",
      identifier: email,
      limit: 3,
      windowMs: 30 * 60 * 1000,
    });

    const admin = createAdminSupabaseClient();
    const appUrl = getOptionalEnv("NEXT_PUBLIC_APP_URL") ?? "http://localhost:3000";
    const { error } = await admin.auth.admin.inviteUserByEmail(email, {
      redirectTo: `${appUrl}/accept-invite`,
      data: {
        full_name: input.fullName ?? "",
      },
    });

    if (error) {
      logWarn("auth.invite.failed", {
        email,
      });
      return {
        success: false,
        message: "Nao foi possivel enviar o convite agora.",
      };
    }

    logInfo("auth.invite.sent", {
      email,
    });

    return {
      success: true,
      message: "Convite enviado com sucesso.",
    };
  } catch (error) {
    if (error instanceof AppError) {
      logWarn("auth.invite.app_error", {
        statusCode: error.statusCode,
        errorCode: error.code,
      });
      return {
        success: false,
        message: error.message,
      };
    }

    logError("auth.invite.failed_unexpected", error);
    return {
      success: false,
      message: "Nao foi possivel enviar o convite agora.",
    };
  }
}

export async function signOutAction() {
  const user = await getCurrentUser();
  const supabase = await createServerSupabaseClient();
  await supabase.auth.signOut();

  if (user) {
    await writeAuditLog({
      userId: user.id,
      action: "logout",
      entityType: "auth",
      entityId: user.id,
    });
  }

  redirect("/login");
}

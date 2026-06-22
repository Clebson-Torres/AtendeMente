UPDATE auth_users SET onboarding_completed = 1, updated_at = datetime('now') WHERE onboarding_completed = 0;

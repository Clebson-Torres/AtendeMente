import { useState, useEffect } from "react";
import { useNavigate, Navigate } from "react-router-dom";
import { getPendingRecoverySecret, clearPendingRecoverySecret, completeOnboarding } from "../lib/auth";
import { useAuth } from "../App";
import WelcomeSecurityScreen from "../components/onboarding/WelcomeSecurityScreen";
import RecoverySecretScreen from "../components/onboarding/RecoverySecretScreen";
import BackupReadyScreen from "../components/onboarding/BackupReadyScreen";

export default function OnboardingFlow() {
  const navigate = useNavigate();
  const { user, loading } = useAuth();
  const [step, setStep] = useState(1);

  useEffect(() => {
    if (!loading && user && user.onboarding_completed) {
      navigate("/", { replace: true });
    }
  }, [user, loading, navigate]);

  if (loading) return null;
  if (!user) return <Navigate to="/login" replace />;
  if (user.onboarding_completed) return <Navigate to="/" replace />;

  const recoverySecret = getPendingRecoverySecret();

  function handleStep1Next() {
    setStep(recoverySecret ? 2 : 3);
  }

  async function handleComplete() {
    await completeOnboarding();
    // useEffect on user.onboarding_completed will navigate to "/"
  }

  function handleBackupSkip() {
    clearPendingRecoverySecret();
  }

  return (
    <div className="flex h-screen items-center justify-center bg-background p-4">
      {step === 1 && <WelcomeSecurityScreen onNext={handleStep1Next} />}
      {step === 2 && recoverySecret && (
        <RecoverySecretScreen
          userId={user.uid}
          secret={recoverySecret}
          onNext={() => setStep(3)}
        />
      )}
      {step === 3 && (
        <BackupReadyScreen
          onComplete={handleComplete}
          onSkip={handleBackupSkip}
        />
      )}
    </div>
  );
}

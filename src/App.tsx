import { useState, useEffect, createContext, useContext } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { onAuthChange, restoreSession, lock } from "./lib/auth";
import Login from "./pages/Login";
import Register from "./pages/Register";
import OnboardingFlow from "./pages/OnboardingFlow";
import Dashboard from "./pages/Dashboard";
import Patients from "./pages/Patients";
import PatientDetail from "./pages/PatientDetail";
import Appointments from "./pages/Appointments";
import AppointmentDetail from "./pages/AppointmentDetail";
import Payments from "./pages/Payments";
import NetworkInfo from "./pages/NetworkInfo";
import Settings from "./pages/Settings";
import Layout from "./components/Layout";
import ToastContainer from "./components/ui/Toast";
import LockScreen from "./components/LockScreen";
import Skeleton from "./components/ui/Skeleton";

export interface AuthUser {
  uid: string;
  email: string | null;
  onboarding_completed: boolean;
}

interface AuthCtx {
  user: AuthUser | null;
  loading: boolean;
}

const AuthContext = createContext<AuthCtx>({ user: null, loading: true });
export const useAuth = () => useContext(AuthContext);

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  if (loading) return <div className="flex h-screen items-center justify-center"><Skeleton className="h-8 w-48" /></div>;
  if (!user) return <Navigate to="/login" replace />;
  if (!user.onboarding_completed) return <Navigate to="/onboarding" replace />;
  return <>{children}</>;
}

const IDLE_TIMEOUT_MS = 5 * 60 * 1000;

export default function App() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [locked, setLocked] = useState(false);

  useEffect(() => {
    let unsub: (() => void) | null = null;
    restoreSession().finally(() => {
      unsub = onAuthChange((u) => {
        setUser(u);
        setLoading(false);
        if (!u) setLocked(false);
      });
    });
    return () => { if (unsub) unsub(); };
  }, []);

  useEffect(() => {
    if (!user || locked) return;

    let timeout: ReturnType<typeof setTimeout>;

    const resetTimer = () => {
      clearTimeout(timeout);
      timeout = setTimeout(async () => {
        try {
          await lock();
        } catch {
          // ignore — session may be expired
        }
        setLocked(true);
      }, IDLE_TIMEOUT_MS);
    };

    const events = ["mousedown", "mousemove", "keydown", "touchstart", "wheel", "scroll"];
    for (const ev of events) {
      window.addEventListener(ev, resetTimer, { passive: true });
    }
    resetTimer();

    return () => {
      clearTimeout(timeout);
      for (const ev of events) {
        window.removeEventListener(ev, resetTimer);
      }
    };
  }, [user, locked]);

  return (
    <AuthContext.Provider value={{ user, loading }}>
      {locked && <LockScreen onUnlock={() => setLocked(false)} />}
      <ToastContainer />
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/register" element={<Register />} />
        <Route path="/onboarding" element={<OnboardingFlow />} />
        <Route
          path="/*"
          element={
            <ProtectedRoute>
              <Layout>
                <Routes>
                  <Route path="/" element={<Dashboard />} />
                  <Route path="/patients" element={<Patients />} />
                  <Route path="/patients/:id" element={<PatientDetail />} />
                  <Route path="/appointments" element={<Appointments />} />
                  <Route path="/appointments/:id" element={<AppointmentDetail />} />
                  <Route path="/payments" element={<Payments />} />
                  <Route path="/network" element={<NetworkInfo />} />
                  <Route path="/settings" element={<Settings />} />
                </Routes>
              </Layout>
            </ProtectedRoute>
          }
        />
      </Routes>
    </AuthContext.Provider>
  );
}

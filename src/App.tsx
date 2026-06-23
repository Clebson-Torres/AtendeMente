import { useState, useEffect, useRef, createContext, useContext, Suspense, lazy } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { onAuthChange, restoreSession, lock } from "./lib/auth";
import Layout from "./components/Layout";
import ToastContainer from "./components/ui/Toast";
import LockScreen from "./components/LockScreen";
import Skeleton from "./components/ui/Skeleton";

const Login = lazy(() => import("./pages/Login"));
const Register = lazy(() => import("./pages/Register"));
const OnboardingFlow = lazy(() => import("./pages/OnboardingFlow"));
const Dashboard = lazy(() => import("./pages/Dashboard"));
const Patients = lazy(() => import("./pages/Patients"));
const PatientDetail = lazy(() => import("./pages/PatientDetail"));
const Appointments = lazy(() => import("./pages/Appointments"));
const AppointmentDetail = lazy(() => import("./pages/AppointmentDetail"));
const Payments = lazy(() => import("./pages/Payments"));
const NetworkInfo = lazy(() => import("./pages/NetworkInfo"));
const Settings = lazy(() => import("./pages/Settings"));
const NotFound = lazy(() => import("./pages/NotFound"));

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
  const isMounted = useRef(true);

  useEffect(() => {
    isMounted.current = true;
    let unsub: (() => void) | null = null;
    restoreSession().finally(() => {
      if (!isMounted.current) return;
      unsub = onAuthChange((u) => {
        if (!isMounted.current) return;
        setUser(u);
        setLoading(false);
        if (!u) setLocked(false);
      });
    });
    return () => {
      isMounted.current = false;
      if (unsub) unsub();
    };
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
      <Suspense fallback={<div className="flex h-screen items-center justify-center"><Skeleton className="h-8 w-48" /></div>}>
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route path="/register" element={<Register />} />
          <Route path="/onboarding" element={<OnboardingFlow />} />
          <Route
            path="/*"
            element={
              <ProtectedRoute>
                <Layout>
                  <Suspense fallback={<div className="flex h-64 items-center justify-center"><Skeleton className="h-8 w-48" /></div>}>
                    <Routes>
                      <Route path="/" element={<Dashboard />} />
                      <Route path="/patients" element={<Patients />} />
                      <Route path="/patients/:id" element={<PatientDetail />} />
                      <Route path="/appointments" element={<Appointments />} />
                      <Route path="/appointments/:id" element={<AppointmentDetail />} />
                      <Route path="/payments" element={<Payments />} />
                      <Route path="/network" element={<NetworkInfo />} />
                      <Route path="/settings" element={<Settings />} />
                      <Route path="*" element={<NotFound />} />
                    </Routes>
                  </Suspense>
                </Layout>
              </ProtectedRoute>
            }
          />
        </Routes>
      </Suspense>
    </AuthContext.Provider>
  );
}

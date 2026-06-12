import { useState, useEffect, createContext, useContext } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { onAuthChange, restoreSession } from "./lib/auth";
import Login from "./pages/Login";
import Register from "./pages/Register";
import Dashboard from "./pages/Dashboard";
import Patients from "./pages/Patients";
import PatientDetail from "./pages/PatientDetail";
import Appointments from "./pages/Appointments";
import AppointmentDetail from "./pages/AppointmentDetail";
import Payments from "./pages/Payments";
import Layout from "./components/Layout";
import ToastContainer from "./components/ui/Toast";

export interface AuthUser {
  uid: string;
  email: string | null;
}

interface AuthCtx {
  user: AuthUser | null;
  loading: boolean;
}

const AuthContext = createContext<AuthCtx>({ user: null, loading: true });
export const useAuth = () => useContext(AuthContext);

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();
  if (loading) return <div className="flex h-screen items-center justify-center text-gray-500">Carregando...</div>;
  if (!user) return <Navigate to="/login" replace />;
  return <>{children}</>;
}

export default function App() {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let unsub: (() => void) | null = null;
    restoreSession().finally(() => {
      unsub = onAuthChange((u) => {
        setUser(u);
        setLoading(false);
      });
    });
    return () => { if (unsub) unsub(); };
  }, []);

  return (
    <AuthContext.Provider value={{ user, loading }}>
      <ToastContainer />
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/register" element={<Register />} />
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
                </Routes>
              </Layout>
            </ProtectedRoute>
          }
        />
      </Routes>
    </AuthContext.Provider>
  );
}

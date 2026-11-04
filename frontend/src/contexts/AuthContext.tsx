import { createContext, useContext, useState, useCallback, ReactNode, useEffect } from "react";
import { AUTH_INVALID_EVENT, loginRequest, ssoRequest, validateSession } from "@/lib/api";

interface AuthState {
  authenticated: boolean;
  username: string;
  token: string;
  role: string;
}

interface AuthContextType {
  auth: AuthState;
  /** True while the UUUUI single-sign-on handoff is being attempted on load. */
  bootstrapping: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextType | null>(null);
const defaultAuthState: AuthState = {
  authenticated: false,
  username: "",
  token: "",
  role: "",
};

export const AuthProvider = ({ children }: { children: ReactNode }) => {
  const [auth, setAuth] = useState<AuthState>(() => {
    const saved = sessionStorage.getItem("infra-av-auth");
    if (!saved) return defaultAuthState;

    try {
      return JSON.parse(saved) as AuthState;
    } catch {
      return defaultAuthState;
    }
  });

  // Attempt UUUUI SSO once on load if there is no existing session: ask the
  // broker (same origin) for a short-lived identity token and exchange it for
  // an INFRA-AV session, so the embedded tab doesn't prompt for a second login.
  const [bootstrapping, setBootstrapping] = useState<boolean>(() => {
    const saved = sessionStorage.getItem("infra-av-auth");
    return !saved; // only bootstrap when we have no stored session
  });

  useEffect(() => {
    if (!bootstrapping) return;
    let cancelled = false;
    void (async () => {
      try {
        const res = await fetch("/api/session/token", { credentials: "same-origin" });
        if (res.ok) {
          const body = (await res.json()) as { token?: string };
          if (body.token) {
            const data = await ssoRequest(body.token);
            if (!cancelled) {
              const state: AuthState = {
                authenticated: true,
                username: data.username,
                token: data.token,
                role: data.role,
              };
              sessionStorage.setItem("infra-av-auth", JSON.stringify(state));
              setAuth(state);
            }
          }
        }
      } catch {
        /* fall back to the manual login form */
      } finally {
        if (!cancelled) setBootstrapping(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bootstrapping]);

  const login = useCallback(async (username: string, password: string) => {
    const data = await loginRequest(username, password);
    const state: AuthState = {
      authenticated: true,
      username: data.username,
      token: data.token,
      role: data.role,
    };
    sessionStorage.setItem("infra-av-auth", JSON.stringify(state));
    setAuth(state);
  }, []);

  const logout = useCallback(() => {
    sessionStorage.removeItem("infra-av-auth");
    setAuth(defaultAuthState);
  }, []);

  useEffect(() => {
    const handleInvalidAuth = () => {
      sessionStorage.removeItem("infra-av-auth");
      setAuth(defaultAuthState);
    };

    window.addEventListener(AUTH_INVALID_EVENT, handleInvalidAuth);
    return () => {
      window.removeEventListener(AUTH_INVALID_EVENT, handleInvalidAuth);
    };
  }, []);

  useEffect(() => {
    if (!auth.authenticated || !auth.token) return;
    void validateSession(auth.token).catch(() => undefined);
  }, [auth.authenticated, auth.token]);

  return (
    <AuthContext.Provider value={{ auth, bootstrapping, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
};

import NextAuth, { NextAuthOptions } from "next-auth";
import CredentialsProvider from "next-auth/providers/credentials";
import { auth as backendAuthApi, BackendApiError } from "@/server/adapter/backend-api.adapter";

const USERINFO_REFRESH_MS = 60 * 1000;   // refresh isGlobalAdmin etc. every 60s
const TOKEN_REFRESH_MARGIN_S = 300;      // refresh access token when < 5 min remaining

// Extend next-auth types to carry the backend JWT and user metadata
declare module "next-auth" {
    interface User {
        backendToken: string;
        refreshToken: string;
        isGlobalAdmin: boolean;
        username: string;
    }
    interface Session {
        backendToken: string;
        user: {
            id: string;
            email: string;
            username: string;
            isGlobalAdmin: boolean;
        };
    }
}
declare module "next-auth/jwt" {
    interface JWT {
        backendToken: string;
        refreshToken: string;
        isGlobalAdmin: boolean;
        username: string;
        userId: string;
        refreshedAt?: number;        // last userinfo refresh
        tokenRefreshedAt?: number;   // last token rotation (debounce)
    }
}

/** Decode JWT exp claim without verification (payload is base64url). */
function getTokenExp(jwt: string): number {
    try {
        const payload = jwt.split('.')[1];
        if (!payload) return 0;
        const decoded = JSON.parse(
            Buffer.from(payload.replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString('utf8'),
        );
        return typeof decoded.exp === 'number' ? decoded.exp : 0;
    } catch {
        return 0;
    }
}

export const authOptions: NextAuthOptions = {
    session: { strategy: "jwt" },
    pages: { signIn: "/auth" },

    providers: [
        CredentialsProvider({
            name: "Credentials",
            credentials: {
                username: { label: "Username", type: "text" },
                password: { label: "Password", type: "password" },
            },
            async authorize(credentials) {
                if (!credentials?.username || !credentials?.password) return null;
                try {
                    const { token, refresh_token, user } = await backendAuthApi.login(
                        credentials.username,
                        credentials.password,
                    );
                    return {
                        id: user.id,
                        email: user.email,
                        username: user.username,
                        backendToken: token,
                        refreshToken: refresh_token,
                        isGlobalAdmin: user.is_global_admin,
                    };
                } catch (err) {
                    if (err instanceof BackendApiError) {
                        throw new Error(err.message);
                    }
                    return null;
                }
            },
        }),
    ],

    callbacks: {
        async jwt({ token, user }) {
            // `user` is only populated on the first sign-in
            if (user) {
                token.userId = user.id;
                token.username = (user as any).username;
                token.backendToken = (user as any).backendToken;
                token.refreshToken = (user as any).refreshToken;
                token.isGlobalAdmin = (user as any).isGlobalAdmin;
                token.refreshedAt = Date.now();
                token.tokenRefreshedAt = Date.now();
            }

            // ── Auto-rotate access token before it expires ──────────────
            // Only attempt once per 60s to avoid hammering the backend
            // when multiple requests arrive in the same window.
            const now = Date.now();
            const sinceLastRotation = now - (token.tokenRefreshedAt ?? 0);

            if (token.backendToken && sinceLastRotation > 60_000) {
                const exp = getTokenExp(token.backendToken);
                const remaining = exp - Math.floor(now / 1000);

                if (remaining < TOKEN_REFRESH_MARGIN_S && token.refreshToken) {
                    try {
                        const result = await backendAuthApi.refresh(token.refreshToken);
                        token.backendToken = result.token;
                        token.refreshToken = result.refresh_token;
                        token.tokenRefreshedAt = now;
                    } catch {
                        // Refresh failed (refresh token expired / revoked).
                        // Clear tokens — getBackendToken() will redirect to /auth.
                        token.backendToken = '';
                        token.refreshToken = '';
                    }
                }
            }

            // ── Periodically refresh user info (isGlobalAdmin etc.) ─────
            const age = now - (token.refreshedAt ?? 0);
            if (age > USERINFO_REFRESH_MS && token.backendToken) {
                try {
                    const me = await backendAuthApi.me(token.backendToken);
                    token.isGlobalAdmin = me.is_global_admin;
                    token.username = me.username;
                    token.refreshedAt = now;
                } catch {
                    // Backend unreachable — keep stale values
                }
            }

            return token;
        },

        async session({ session, token }) {
            session.backendToken = token.backendToken;
            session.user = {
                id: token.userId,
                email: token.email ?? "",
                username: token.username,
                isGlobalAdmin: token.isGlobalAdmin,
            };
            return session;
        },
    },
};

import NextAuth, { NextAuthOptions } from "next-auth";
import CredentialsProvider from "next-auth/providers/credentials";
import { auth as backendAuthApi, BackendApiError } from "@/server/adapter/backend-api.adapter";

// Extend next-auth types to carry the backend JWT and user metadata
declare module "next-auth" {
    interface User {
        backendToken: string;
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
        isGlobalAdmin: boolean;
        username: string;
        userId: string;
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
                totpToken: { label: "TOTP Token", type: "text" },
            },
            async authorize(credentials) {
                if (!credentials?.username || !credentials?.password) return null;
                try {
                    const { token, user } = await backendAuthApi.login(
                        credentials.username,
                        credentials.password,
                        credentials.totpToken || undefined,
                    );
                    return {
                        id: user.id,
                        email: user.email,
                        username: user.username,
                        backendToken: token,
                        isGlobalAdmin: user.is_global_admin,
                    };
                } catch (err) {
                    if (err instanceof BackendApiError) {
                        // Surface the backend error message to the login form
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
                token.isGlobalAdmin = (user as any).isGlobalAdmin;
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

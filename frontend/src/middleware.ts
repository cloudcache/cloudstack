export { default } from "next-auth/middleware";

// Protect the authenticated areas (console + admin). Unauthenticated users are
// redirected to the NextAuth signIn page (/auth). Admin-only enforcement (the
// is_global_admin flag) lives in the (admin) route-group layout. The public
// portal tier (/, /auth, /verify-email, /error, /unauthorized) is not matched.
export const config = {
  matcher: [
    "/projects/:path*",
    "/project/:path*",
    "/billing/:path*",
    "/monitoring/:path*",
    "/backups/:path*",
    "/plans/:path*",
    "/settings/:path*",
    "/resources/:path*",
    "/business/:path*",
  ],
};

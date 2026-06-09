import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

/** @type {import('next').NextConfig} */
const nextConfig = {
    output: 'standalone',
    outputFileTracingRoot: __dirname,
    typescript: {
        // Type-checking is enforced again. `tsc --noEmit` is clean — the
        // remaining `as any` casts paper over Prisma→Rust camel/snake
        // mismatches; removing a cast must be paired with a real type fix.
        ignoreBuildErrors: false,
    },
    eslint: {
        // Enforced again. `next lint` reports 0 errors (53 react-hooks
        // exhaustive-deps warnings remain — warnings don't fail the build).
        ignoreDuringBuilds: false,
    },
   /* experimental: {
        instrumentationHook: true
    }*/
};

export default nextConfig;

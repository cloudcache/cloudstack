import Link from "next/link";
import { Button } from "@/components/ui/button";
import QuickStackLogo from "@/components/custom/quickstack-logo";
import { getUserSession } from "@/server/utils/action-wrapper.utils";
import { redirect } from "next/navigation";

// Public marketing / landing page (URL: /).
// Logged-in users are sent straight to their console.
export default async function PortalLanding() {
  const session = await getUserSession();
  if (session) {
    redirect("/projects");
  }

  return (
    <div className="flex min-h-screen flex-col items-center justify-center px-6 text-center">
      <div className="flex aspect-square size-14 items-center justify-center rounded-2xl bg-qs-500">
        <QuickStackLogo className="size-9" color="light-all" />
      </div>
      <h1 className="mt-6 text-4xl font-bold tracking-tight sm:text-5xl">QuickStack</h1>
      <p className="mt-4 max-w-xl text-lg text-muted-foreground">
        Deploy and run your apps, databases and services on your own clusters — one platform for
        provisioning, deployment, scaling, and billing.
      </p>
      <div className="mt-8 flex gap-3">
        <Button asChild size="lg">
          <Link href="/auth">Sign in</Link>
        </Button>
        <Button asChild size="lg" variant="outline">
          <Link href="https://quickstack.dev/docs" target="_blank">Documentation</Link>
        </Button>
      </div>
    </div>
  );
}

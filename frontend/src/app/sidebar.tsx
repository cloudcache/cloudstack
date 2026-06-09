import { getUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils"
import { SidebarCient, SidebarTier } from "./sidebar-client"
import { backend } from "@/server/adapter/backend-api.adapter"

export async function AppSidebar({ tier }: { tier: SidebarTier }) {

  const session = await getUserSession();

  if (!session) {
    return <></>
  }
  let projects: { id: string; name: string }[] = [];
  // The project list is only used by the console sidebar.
  if (tier === 'console') {
    try {
      const token = await getBackendToken();
      projects = (await backend.projects.list(token)) as { id: string; name: string }[];
    } catch {
      // If backend is unreachable, render sidebar with empty project list
    }
  }

  return <SidebarCient projects={projects} session={session} tier={tier} />
}

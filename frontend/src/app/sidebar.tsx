import { getUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils"
import { SidebarCient } from "./sidebar-client"
import { backend } from "@/server/adapter/backend-api.adapter"

export async function AppSidebar() {

  const session = await getUserSession();

  if (!session) {
    return <></>
  }
  let projects: { id: string; name: string }[] = [];
  try {
    const token = await getBackendToken();
    projects = (await backend.projects.list(token)) as { id: string; name: string }[];
  } catch {
    // If backend is unreachable, render sidebar with empty project list
  }

  return <SidebarCient projects={projects} session={session} />
}

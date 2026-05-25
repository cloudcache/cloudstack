'use client'

import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarHeader,
  SidebarFooter,
  SidebarMenuAction,
  SidebarMenuSub,
  SidebarMenuSubItem,
  useSidebar
} from "@/components/ui/sidebar"
import { BookOpen, ChartNoAxesCombined, ChevronDown, ChevronUp, CreditCard, Dot, FolderClosed, History, Info, Plus, Settings2, User, User2 } from "lucide-react"
import Link from "next/link"
import { EditProjectDialog } from "./projects/edit-project-dialog"
import { SidebarLogoutButton } from "./sidebar-logout-button"
import {
  Avatar,
  AvatarFallback,
} from "@/components/ui/avatar"
import { UserSession } from "@/shared/model/sim-session.model"
import { usePathname } from "next/navigation"
import { useEffect, useState } from "react"
import QuickStackLogo from "@/components/custom/quickstack-logo"
import { UserGroupUtils } from "@/shared/utils/role.utils"

interface SidebarProject {
  id: string;
  name: string;
}

export function SidebarCient({
  projects,
  session,
}: {
  projects: SidebarProject[];
  session: UserSession;
}) {

  const path = usePathname();

  const [currentlySelectedProjectId, setCurrentlySelectedProjectId] = useState<string | null>(null);

  const settingsMenu = [
    {
      title: "Profile",
      url: "/settings/profile",
      icon: User,
    },
    {
      title: "Users & Groups",
      url: "/settings/users",
      icon: User2,
      adminOnly: true,
    },
    {
      title: "S3 Targets",
      url: "/settings/s3-targets",
      icon: Settings,
      adminOnly: true,
    },
    {
      title: <span>QuickStack Settings</span>,
      url: "/settings/server",
      adminOnly: true,
    },
  ]

  useEffect(() => {
    if (path.startsWith('/project')) {
      const projectId = path.split('/')[2];
      setCurrentlySelectedProjectId(projectId || null);
    } else {
      setCurrentlySelectedProjectId(null);
    }
  }, [path]);

  const { open } = useSidebar()

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton size="lg"
                  className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground">
                  <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-qs-500 text-sidebar-primary-foreground">
                    <QuickStackLogo className="size-5" color="light-all" />
                  </div>
                  <div className="grid flex-1 text-left text-sm leading-tight my-4">
                    <span className="truncate font-semibold">QuickStack</span>
                    <span className="truncate text-xs">Admin Panel</span>
                  </div>
                  <ChevronDown className="ml-auto" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent className="w-[--radix-popper-anchor-width]">
                <Link href="https://quickstack.dev" target="_blank">
                  <DropdownMenuItem>
                    <Info />
                    <span>QuickStack Website</span>
                  </DropdownMenuItem>
                </Link>
                <Link href="https://quickstack.dev/docs" target="_blank">
                  <DropdownMenuItem>
                    <BookOpen />
                    <span>QuickStack Docs</span>
                  </DropdownMenuItem>
                </Link>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Menu</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{
                  children: 'All Projects',
                  hidden: open,
                }}
                  isActive={path === '/'}>
                  <Link href="/">
                    <FolderClosed />
                    <span>Projects</span>
                  </Link>
                </SidebarMenuButton>
                {UserGroupUtils.isAdmin(session) && <EditProjectDialog>
                  <SidebarMenuAction>
                    <Plus />
                  </SidebarMenuAction>
                </EditProjectDialog>}
                <SidebarMenu>
                  {projects.map((item) => (
                    <SidebarMenuItem key={item.id}>
                      <SidebarMenuButton asChild tooltip={{
                        children: `Project: ${item.name}`,
                        hidden: open,
                      }}
                        isActive={currentlySelectedProjectId === item.id}
                      >
                        <Link href={`/project/${item.id}`}>
                          <Dot /> <span>{item.name}</span>
                        </Link>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{
                  children: 'Monitoring',
                  hidden: open,
                }}
                  isActive={path.startsWith('/monitoring')}>
                  <Link href="/monitoring">
                    <ChartNoAxesCombined />
                    <span>Monitoring</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>


        {UserGroupUtils.sessionHasAccessToBackups(session) && <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{
                  children: 'Backups',
                  hidden: open,
                }}
                  isActive={path.startsWith('/backups')}>
                  <Link href="/backups">
                    <History />
                    <span>Backups</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>}

        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{
                  children: 'Billing',
                  hidden: open,
                }}
                  isActive={path.startsWith('/billing')}>
                  <Link href="/billing">
                    <CreditCard />
                    <span>Billing</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>


        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{
                  children: 'Settings',
                  hidden: open,
                }}>
                  <Link href="/settings/profile">
                    <Settings2 />
                    <span>Settings</span>
                  </Link>
                </SidebarMenuButton>
                <SidebarMenuSub>
                  {(UserGroupUtils.isAdmin(session) ? settingsMenu :
                    settingsMenu.filter(x => !x.adminOnly)).map((item) => (
                      <SidebarMenuSubItem key={item.url}>
                        <SidebarMenuButton asChild>
                          <Link href={item.url}>
                            <span>{item.title}</span>
                          </Link>
                        </SidebarMenuButton>
                      </SidebarMenuSubItem>
                    ))}
                </SidebarMenuSub>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton
                  size="lg"
                  className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground">
                  <Avatar className="h-8 w-8 rounded-lg">
                    <AvatarFallback className="rounded-lg">{session.email.substring(0, 1)?.toUpperCase() || 'Q'}</AvatarFallback>
                  </Avatar>
                  {session.email}
                  <ChevronUp className="ml-auto" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                side="top"
                className="w-[--radix-popper-anchor-width]"
              >
                <Link href="/settings/profile">
                  <DropdownMenuItem>
                    <User />
                    <span>Profile</span>
                  </DropdownMenuItem>
                </Link>
                <SidebarLogoutButton />
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  )
}

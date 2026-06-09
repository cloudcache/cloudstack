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
import { BookOpen, Boxes, ChartNoAxesCombined, ChevronDown, ChevronUp, CreditCard, Cpu, Database, Dot, FolderClosed, GitBranch, HardDrive, History, Info, LayoutGrid, Mail, MemoryStick, MessageSquare, Network, Plus, Server, Settings2, Shield, ShieldCheck, Tag, User, User2, Workflow } from "lucide-react"
import Link from "next/link"
import { EditProjectDialog } from "./(console)/projects/edit-project-dialog"
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
import { useT } from "@/i18n"
import { LanguageMenuButton, LanguageSwitcher } from "@/components/i18n/language-switcher"

interface SidebarProject {
  id: string;
  name: string;
}

export type SidebarTier = 'admin' | 'console';

export function SidebarCient({
  projects,
  session,
  tier,
}: {
  projects: SidebarProject[];
  session: UserSession;
  tier: SidebarTier;
}) {

  const path = usePathname();
  const t = useT();

  const [currentlySelectedProjectId, setCurrentlySelectedProjectId] = useState<string | null>(null);

  const isAdmin = UserGroupUtils.isAdmin(session);

  const resourceMenu = [
    { title: t("nav.resourcePools"), url: "/resources/pools", icon: Cpu },
    { title: t("nav.clusters"), url: "/resources/clusters", icon: HardDrive },
    { title: t("nav.nodes"), url: "/resources/nodes", icon: Server },
    { title: t("nav.network"), url: "/resources/network", icon: Network },
    { title: t("nav.storageS3Targets"), url: "/resources/storage/s3-targets", icon: Boxes },
    { title: t("nav.loadBalancing"), url: "/resources/load-balancing", icon: GitBranch },
    { title: t("nav.registries"), url: "/resources/registries", icon: Shield },
    { title: t("nav.databaseClusters"), url: "/resources/databases", icon: Database },
    { title: t("nav.mqEndpoints"), url: "/resources/mq-endpoints", icon: MessageSquare },
    { title: t("nav.smtpEndpoints"), url: "/resources/smtp-endpoints", icon: Mail },
    { title: t("nav.redisEndpoints"), url: "/resources/redis-endpoints", icon: MemoryStick },
    { title: t("nav.appTemplates"), url: "/resources/app-templates", icon: Workflow },
    { title: t("nav.platformConfig"), url: "/resources/platform-config", icon: Settings2 },
  ];

  const businessMenu = [
    { title: t("nav.subscriptionPlans"), url: "/business/plans", icon: Tag },
    { title: t("nav.finance"), url: "/business/billing", icon: CreditCard },
  ];

  useEffect(() => {
    if (path.startsWith('/project')) {
      const projectId = path.split('/')[2];
      setCurrentlySelectedProjectId(projectId || null);
    } else {
      setCurrentlySelectedProjectId(null);
    }
  }, [path]);

  const { open } = useSidebar()

  const renderMenu = (items: { title: string; url: string; icon: any }[]) =>
    items.map((item) => (
      <SidebarMenuItem key={item.url}>
        <SidebarMenuButton asChild tooltip={{ children: item.title, hidden: open }}
          isActive={path.startsWith(item.url)}>
          <Link href={item.url}>
            <item.icon />
            <span>{item.title}</span>
          </Link>
        </SidebarMenuButton>
      </SidebarMenuItem>
    ));

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
                    <span className="truncate text-xs">{tier === 'admin' ? t("nav.adminPanel") : t("nav.console")}</span>
                  </div>
                  <ChevronDown className="ml-auto" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent className="w-[--radix-popper-anchor-width]">
                <Link href="https://quickstack.dev" target="_blank">
                  <DropdownMenuItem>
                    <Info />
                    <span>{t("nav.website")}</span>
                  </DropdownMenuItem>
                </Link>
                <Link href="https://quickstack.dev/docs" target="_blank">
                  <DropdownMenuItem>
                    <BookOpen />
                    <span>{t("nav.docs")}</span>
                  </DropdownMenuItem>
                </Link>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        {tier === 'console' && <>
          <SidebarGroup>
            <SidebarGroupLabel>{t("nav.menu")}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip={{ children: t("nav.projects"), hidden: open }}
                    isActive={path === '/projects'}>
                    <Link href="/projects">
                      <FolderClosed />
                      <span>{t("nav.projects")}</span>
                    </Link>
                  </SidebarMenuButton>
                  <EditProjectDialog>
                    <SidebarMenuAction>
                      <Plus />
                    </SidebarMenuAction>
                  </EditProjectDialog>
                  <SidebarMenu>
                    {projects.map((item) => (
                      <SidebarMenuItem key={item.id}>
                        <SidebarMenuButton asChild tooltip={{ children: t("nav.projectTooltip", { name: item.name }), hidden: open }}
                          isActive={currentlySelectedProjectId === item.id}>
                          <Link href={`/project/${item.id}`}>
                            <Dot /> <span>{item.name}</span>
                          </Link>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    ))}
                  </SidebarMenu>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip={{ children: t("nav.monitoring"), hidden: open }}
                    isActive={path.startsWith('/monitoring')}>
                    <Link href="/monitoring">
                      <ChartNoAxesCombined />
                      <span>{t("nav.monitoring")}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                {UserGroupUtils.sessionHasAccessToBackups(session) &&
                  <SidebarMenuItem>
                    <SidebarMenuButton asChild tooltip={{ children: t("nav.backups"), hidden: open }}
                      isActive={path.startsWith('/backups')}>
                      <Link href="/backups">
                        <History />
                        <span>{t("nav.backups")}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>

          <SidebarGroup>
            <SidebarGroupLabel>{t("nav.billing")}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip={{ children: t("nav.plans"), hidden: open }}
                    isActive={path.startsWith('/plans')}>
                    <Link href="/plans">
                      <Tag />
                      <span>{t("nav.plans")}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip={{ children: t("nav.billing"), hidden: open }}
                    isActive={path.startsWith('/billing')}>
                    <Link href="/billing">
                      <CreditCard />
                      <span>{t("nav.billing")}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </>}

        {tier === 'admin' && <>
          <SidebarGroup>
            <SidebarGroupLabel>{t("nav.resourceManagement")}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>{renderMenu(resourceMenu)}</SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>

          <SidebarGroup>
            <SidebarGroupLabel>{t("nav.businessManager")}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {renderMenu(businessMenu)}
                <SidebarMenuItem>
                  <SidebarMenuButton asChild tooltip={{ children: t("nav.usersGroups"), hidden: open }}
                    isActive={path.startsWith('/settings/users')}>
                    <Link href="/settings/users">
                      <User2 />
                      <span>{t("nav.usersGroups")}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </>}

        {/* Cross-tier switch (admins only) */}
        {isAdmin && <SidebarGroup className="mt-auto">
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                {tier === 'console'
                  ? <SidebarMenuButton asChild tooltip={{ children: t("nav.adminPanel"), hidden: open }}>
                    <Link href="/resources/pools">
                      <ShieldCheck />
                      <span>{t("nav.adminPanel")}</span>
                    </Link>
                  </SidebarMenuButton>
                  : <SidebarMenuButton asChild tooltip={{ children: t("nav.console"), hidden: open }}>
                    <Link href="/projects">
                      <LayoutGrid />
                      <span>{t("nav.console")}</span>
                    </Link>
                  </SidebarMenuButton>}
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>}

        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild tooltip={{ children: t("nav.settings"), hidden: open }}
                  isActive={path.startsWith('/settings/profile')}>
                  <Link href="/settings/profile">
                    <Settings2 />
                    <span>{t("nav.settings")}</span>
                  </Link>
                </SidebarMenuButton>
                <SidebarMenuSub>
                  <SidebarMenuSubItem>
                    <SidebarMenuButton asChild>
                      <Link href="/settings/profile">
                        <span>{t("nav.profile")}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuSubItem>
                </SidebarMenuSub>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <LanguageMenuButton />
          </SidebarMenuItem>
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
                    <span>{t("nav.profile")}</span>
                  </DropdownMenuItem>
                </Link>
                <LanguageSwitcher />
                <SidebarLogoutButton />
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  )
}

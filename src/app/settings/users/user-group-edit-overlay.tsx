'use client'

import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form"
import { Input } from "@/components/ui/input"
import { zodResolver } from "@hookform/resolvers/zod"
import { useForm } from "react-hook-form"
import { useActionState, useTransition } from 'react'
import { useEffect, useState } from "react";
import { FormUtils } from "@/frontend/utils/form.utilts";
import { SubmitButton } from "@/components/custom/submit-button";
import { ServerActionResult } from "@/shared/model/server-action-error-return.model"
import { toast } from "sonner"
import { ScrollArea } from "@/components/ui/scroll-area"
import { saveRole } from "./actions"
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts"
import { RoleEditModel, roleEditZodModel } from "@/shared/model/role-edit.model"
import { Checkbox } from "@/components/ui/checkbox"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { ProjectExtendedModel } from "@/shared/model/project-extended.model"
import { UserGroupExtended } from "@/shared/model/sim-session.model"


type UiProjectPermission = {
  projectId: string;
  createApps: boolean;
  deleteApps: boolean;
  writeApps: boolean;
  readApps: boolean;
  setPermissionsPerApp: boolean;
  roleAppPermissions: {
    appId: string;
    appName: string;
    permission?: RolePermissionEnum;
  }[];
};

export default function RoleEditOverlay({ children, userGroup, projects }: {
  children: React.ReactNode;
  userGroup?: UserGroupExtended;
  projects: ProjectExtendedModel[]
}) {

  const [isOpen, setIsOpen] = useState<boolean>(false);

  const [projectPermissions, setProjectPermissions] = useState<UiProjectPermission[]>([]);

  const form = useForm<RoleEditModel>({
    resolver: zodResolver(roleEditZodModel),
    defaultValues: userGroup
  });

  const [, startTransition] = useTransition();
  const [state, formAction] = useActionState((state: ServerActionResult<any, any>,
    payload: RoleEditModel) =>
    saveRole(state, {
      ...payload,
      id: userGroup?.id,
      roleProjectPermissions: projects.map((project) => {
        const projectPermission = projectPermissions.find((perm) => perm.projectId === project.id);
        if (!projectPermission) {
          return undefined;
        }
        return {
          projectId: project.id,
          createApps: projectPermission.createApps,
          deleteApps: projectPermission.deleteApps,
          writeApps: projectPermission.writeApps,
          readApps: projectPermission.readApps,
          roleAppPermissions: projectPermission.roleAppPermissions.filter(ap => !!ap.permission).map((appPerm) => {
            return {
              appId: appPerm.appId,
              permission: appPerm.permission!,
            };
          }),
        }
      }).filter((perm) => perm !== undefined),
    }), FormUtils.getInitialFormState<typeof roleEditZodModel>());

  useEffect(() => {
    if (state.status === 'success') {
      form.reset();
      toast.success('Group saved successfully');
      setIsOpen(false);
    }
    FormUtils.mapValidationErrorsToForm<typeof roleEditZodModel>(state, form);
  }, [state]);

  useEffect(() => {
    if (userGroup) {
      form.reset(userGroup);
      // Initialize app permissions based on role data
      const initialPermissions = projects.map(project => {
        const existingPermission = userGroup.roleProjectPermissions?.find(p => p.projectId === project.id);
        const roleAppPermissions = project.apps.map(app => ({
          appId: app.id,
          appName: app.name,
          permission: existingPermission?.roleAppPermissions.find(appPerm => appPerm.appId === app.id)?.permission
        }));
        const hasNoAppRolePermissionsSet = roleAppPermissions.every(appPerm => !appPerm.permission);
        return {
          projectId: project.id,
          createApps: existingPermission?.createApps || false,
          deleteApps: existingPermission?.deleteApps || false,
          writeApps: existingPermission?.writeApps || false,
          readApps: existingPermission?.readApps || false,
          setPermissionsPerApp: (existingPermission?.roleAppPermissions.length ?? 0) > 0 || false,
          roleAppPermissions: hasNoAppRolePermissionsSet ? [] : roleAppPermissions
        } as UiProjectPermission;
      });
      setProjectPermissions(initialPermissions);
    } else {
      // Initialize with all apps having no permissions
      const initialPermissions = projects.map(project => ({
        projectId: project.id,
        createApps: false,
        deleteApps: false,
        writeApps: false,
        readApps: false,
        setPermissionsPerApp: false,
        roleAppPermissions: []
      } as UiProjectPermission));
      setProjectPermissions(initialPermissions);
    }
  }, [userGroup, projects, isOpen]);


  const handleReadChange = (projectId: string, checked: boolean) => {
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.projectId === projectId) {
        return { ...perm, readApps: checked };
      }
      return perm;
    }));
  };

  const handleReadWriteChange = (projectId: string, checked: boolean) => {
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.projectId === projectId) {
        return { ...perm, writeApps: checked, readApps: checked ? true : perm.writeApps };
      }
      return perm;
    }));
  };

  const handleCreateChange = (projectId: string, checked: boolean) => {
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.projectId === projectId) {
        return { ...perm, createApps: checked, readApps: checked ? true : perm.createApps };
      }
      return perm;
    }));
  };

  const handleDeleteChange = (projectId: string, checked: boolean) => {
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.projectId === projectId) {
        return { ...perm, deleteApps: checked, readApps: checked ? true : perm.deleteApps };
      }
      return perm;
    }));
  };

  const handleSetPermissionsPerAppChange = (projectId: string, checked: boolean) => {
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.projectId === projectId) {
        const appPermissions = checked ? projects.find(p => p.id === projectId)?.apps.map(app => ({
          appId: app.id,
          appName: app.name,
          permission: undefined
        })) || [] : [];
        return {
          ...perm,
          setPermissionsPerApp: checked,
          roleAppPermissions: appPermissions,
          createApps: false,
          deleteApps: false,
          writeApps: false,
          readApps: false
        };
      }
      return perm;
    }));
  };

  const handleAppReadChange = (appId: string, checked: boolean) =>
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.roleAppPermissions.some(appPerm => appPerm.appId === appId)) {
        return {
          ...perm,
          roleAppPermissions: perm.roleAppPermissions.map(appPerm => {
            if (appPerm.appId === appId) {
              return { ...appPerm, permission: checked ? RolePermissionEnum.READ : undefined };
            }
            return appPerm;
          })
        };
      }
      return perm;
    }));

  const handleAppReadWriteChange = (appId: string, checked: boolean) =>
    setProjectPermissions(prev => prev.map(perm => {
      if (perm.roleAppPermissions.some(appPerm => appPerm.appId === appId)) {
        return {
          ...perm,
          roleAppPermissions: perm.roleAppPermissions.map(appPerm => {
            if (appPerm.appId === appId) {
              return { ...appPerm, permission: checked ? RolePermissionEnum.READWRITE : undefined };
            }
            return appPerm;
          })
        };
      }
      return perm;
    }));

  return (
    <>
      <div onClick={() => setIsOpen(true)}>
        {children}
      </div>
      <Dialog open={!!isOpen} onOpenChange={(isOpened) => setIsOpen(isOpened)}>
        <DialogContent className="sm:max-w-[900px]">
          <DialogHeader>
            <DialogTitle>{userGroup?.id ? 'Edit' : 'Create'} Group</DialogTitle>
          </DialogHeader>
          <ScrollArea className="max-h-[70vh]">
            <div className="px-3">
              <Form {...form}>
                <form action={(e) => form.handleSubmit((data) => {
                  return startTransition(() => formAction(data));
                }, console.error)()}>
                  <div className="space-y-4">
                    <FormField
                      control={form.control}
                      name="name"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>Name</FormLabel>
                          <FormControl>
                            <Input placeholder="" {...field} />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <FormField
                      control={form.control}
                      name="canAccessBackups"
                      render={({ field }) => (
                        <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                          <FormControl>
                            <Checkbox
                              checked={field.value}
                              onCheckedChange={field.onChange}
                            />
                          </FormControl>
                          <div className="space-y-1 leading-none">
                            <FormLabel>
                              Can access backups
                            </FormLabel>
                            <FormDescription>
                              If enabled, users can access the backups page and download backups from all apps.
                            </FormDescription>
                          </div>
                        </FormItem>
                      )}
                    />

                    <div className="pt-3">
                      <h3 className="text-sm font-medium mb-2">App Permissions</h3>
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead>Project</TableHead>
                            <TableHead>Individual Permissions</TableHead>
                            <TableHead>Read Apps</TableHead>
                            <TableHead>Edit/Deploy Apps</TableHead>
                            <TableHead>Create Apps</TableHead>
                            <TableHead>Delete Apps</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {projects.map((project) => {
                            const permission = projectPermissions.find(p => p.projectId === project.id);
                            return (
                              <>
                                <TableRow key={project.id} className={(permission?.roleAppPermissions.length ?? 0) === 0 ? 'border-b-gray-400' : ''} >
                                  <TableCell className="font-semibold">{project.name}</TableCell>
                                  <TableCell>
                                    <Checkbox
                                      id={`delete-${project.id}`}
                                      checked={permission?.setPermissionsPerApp || false}
                                      onCheckedChange={(checked) => handleSetPermissionsPerAppChange(project.id, !!checked)}
                                    />
                                  </TableCell>
                                  {permission?.setPermissionsPerApp ?
                                    <TableHead>App</TableHead>
                                    : <TableCell>
                                      <Checkbox
                                        id={`read-${project.id}`}
                                        disabled={permission?.writeApps || permission?.deleteApps || permission?.createApps}
                                        checked={permission?.readApps || false}
                                        onCheckedChange={(checked) => handleReadChange(project.id, !!checked)}
                                      />
                                    </TableCell>}
                                  <TableCell>
                                    {!permission?.setPermissionsPerApp &&
                                      <Checkbox
                                        id={`write-${project.id}`}
                                        checked={permission?.writeApps || false}
                                        onCheckedChange={(checked) => handleReadWriteChange(project.id, !!checked)}
                                      />}
                                  </TableCell>
                                  {permission?.setPermissionsPerApp ?
                                    <TableHead>Read</TableHead>
                                    : <TableCell>
                                      <Checkbox
                                        id={`create-${project.id}`}
                                        checked={permission?.createApps || false}
                                        onCheckedChange={(checked) => handleCreateChange(project.id, !!checked)}
                                      />
                                    </TableCell>}
                                  {permission?.setPermissionsPerApp ?
                                    <TableHead>Read, Write & Deploy</TableHead>
                                    : <TableCell>
                                      <Checkbox
                                        id={`delete-${project.id}`}
                                        checked={permission?.deleteApps || false}
                                        onCheckedChange={(checked) => handleDeleteChange(project.id, !!checked)}
                                      />
                                    </TableCell>}
                                </TableRow>


                                {(permission?.roleAppPermissions.length ?? 0) > 0 &&
                                  <>
                                    {permission?.roleAppPermissions.map((roleAppPermission, index) =>

                                      <TableRow key={roleAppPermission.appId} className={permission.roleAppPermissions.length - 1 === index ? 'border-b-gray-400' : ''}>
                                        <TableCell></TableCell>
                                        <TableCell></TableCell>
                                        <TableCell colSpan={2}>{roleAppPermission.appName}</TableCell>
                                        <TableCell>
                                          <Checkbox
                                            id={`app-read-${roleAppPermission.appId}`}
                                            checked={roleAppPermission.permission === RolePermissionEnum.READ}
                                            onCheckedChange={(checked) => handleAppReadChange(roleAppPermission.appId, !!checked)}
                                          />
                                        </TableCell>
                                        <TableCell>
                                          <Checkbox
                                            id={`app-readwrite-${roleAppPermission.appId}`}
                                            checked={roleAppPermission.permission === RolePermissionEnum.READWRITE}
                                            onCheckedChange={(checked) => handleAppReadWriteChange(roleAppPermission.appId, !!checked)}
                                          />
                                        </TableCell>
                                      </TableRow>

                                    )}
                                  </>}
                              </>
                            );
                          })}
                        </TableBody>
                      </Table>
                    </div>

                    <p className="text-red-500">{state.message}</p>
                    <SubmitButton>Save</SubmitButton>
                  </div>
                </form>
              </Form >
            </div>
          </ScrollArea>
        </DialogContent>
      </Dialog >
    </>
  )
}
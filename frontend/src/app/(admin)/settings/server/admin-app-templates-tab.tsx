'use client'

import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { Plus, Pencil, Trash, Copy } from "lucide-react";
import { createAppTemplate, updateAppTemplate, deleteAppTemplate } from "./actions";
import TemplateEditorDialog, { type EditableTemplate, newBlankTemplate } from "./template-editor-dialog";
import type { TemplateDto } from "@/server/adapter/backend-api.adapter";

export default function AdminAppTemplatesTab({ initialItems }: { initialItems: TemplateDto[] }) {
    const [items, setItems] = useState<TemplateDto[]>(initialItems);
    const [editing, setEditing] = useState<EditableTemplate | null>(null);
    const { openConfirmDialog } = useConfirmDialog();

    const startCreate = () => setEditing(newBlankTemplate());
    const startEdit = (dto: TemplateDto) => setEditing(dtoToEditable(dto));
    const startDuplicate = (dto: TemplateDto) => {
        const copy = dtoToEditable(dto);
        copy.id = undefined;
        copy.slug = `${dto.slug}-copy`;
        copy.name = `${dto.name} (copy)`;
        setEditing(copy);
    };

    const onSave = async (e: EditableTemplate) => {
        const body = editableToUpsert(e);
        if (e.id) {
            const r = await updateAppTemplate(e.id, body);
            if (r?.status === 'success') {
                setItems(prev => prev.map(i => i.id === e.id ? { ...i, ...body } as any : i));
                setEditing(null);
                return true;
            }
        } else {
            const r: any = await createAppTemplate(body);
            if (r?.status === 'success') {
                window.location.reload();
                return true;
            }
        }
        return false;
    };

    const handleDelete = async (dto: TemplateDto) => {
        const confirmed = await openConfirmDialog({
            title: `Delete template ${dto.name}?`,
            description: `This removes "${dto.name}" from the catalog. Existing apps deployed from it are unaffected.`,
            okButton: 'Delete', cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => deleteAppTemplate(dto.id));
        setItems(prev => prev.filter(i => i.id !== dto.id));
    };

    return (
        <>
            <Card>
                <CardHeader className="flex flex-row items-start justify-between">
                    <div>
                        <CardTitle>App Templates</CardTitle>
                        <CardDescription>
                            Catalog of deployable apps. Each template carries an image reference plus optional
                            service requirements (DB / Redis / S3 / MQ / SMTP) injected as env vars or rendered
                            config files at deploy time.
                        </CardDescription>
                    </div>
                    <Button size="sm" onClick={startCreate}><Plus className="mr-2 h-4 w-4" />New template</Button>
                </CardHeader>
                <CardContent>
                    {items.length === 0 ? (
                        <p className="text-muted-foreground text-sm">No templates yet.</p>
                    ) : (
                        <div className="space-y-3">
                            {items.map((it) => (
                                <div key={it.id} className="flex items-center justify-between border rounded-lg p-3">
                                    <div className="flex items-center gap-3 min-w-0">
                                        {it.icon_url && <img src={it.icon_url} className="h-8 w-8 flex-shrink-0" alt="" />}
                                        <div className="min-w-0">
                                            <div className="flex items-center gap-2 flex-wrap">
                                                <span className="font-medium">{it.name}</span>
                                                <span className="text-xs text-muted-foreground">({it.slug})</span>
                                                <Badge variant="outline">{it.category}</Badge>
                                                <Badge variant={it.is_active ? 'default' : 'secondary'}>
                                                    {it.is_active ? 'active' : 'inactive'}
                                                </Badge>
                                                {(it.requirements?.length ?? 0) > 0 &&
                                                    <Badge variant="secondary">{it.requirements.length} deps</Badge>}
                                            </div>
                                            <div className="text-xs font-mono text-muted-foreground truncate">
                                                {it.image_repository}:{it.image_tag}
                                            </div>
                                            {it.description && <div className="text-xs text-muted-foreground truncate">{it.description}</div>}
                                        </div>
                                    </div>
                                    <div className="flex items-center gap-1 flex-shrink-0">
                                        <Button size="sm" variant="ghost" onClick={() => startEdit(it)}>
                                            <Pencil className="h-4 w-4" />
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => startDuplicate(it)}>
                                            <Copy className="h-4 w-4" />
                                        </Button>
                                        <Button size="sm" variant="ghost" onClick={() => handleDelete(it)}>
                                            <Trash className="h-4 w-4 text-destructive" />
                                        </Button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </CardContent>
            </Card>

            {editing && (
                <TemplateEditorDialog
                    template={editing}
                    onClose={() => setEditing(null)}
                    onSave={onSave}
                />
            )}
        </>
    );
}

function dtoToEditable(d: TemplateDto): EditableTemplate {
    return {
        id: d.id,
        slug: d.slug,
        name: d.name,
        icon_url: d.icon_url ?? '',
        category: d.category,
        description: d.description ?? '',
        is_active: d.is_active,
        image_registry_id: d.image_registry_id ?? '',
        image_repository: d.image_repository,
        image_tag: d.image_tag,
        image_digest: d.image_digest ?? '',
        spec: d.spec,
        requirements: (d.requirements ?? []) as any,
        inputs: (d.inputs ?? []) as any,
    };
}

function editableToUpsert(e: EditableTemplate): any {
    return {
        slug: e.slug,
        name: e.name,
        icon_url: e.icon_url || null,
        category: e.category,
        description: e.description || null,
        image_registry_id: e.image_registry_id || null,
        image_repository: e.image_repository,
        image_tag: e.image_tag,
        image_digest: e.image_digest || null,
        spec: e.spec,
        requirements: e.requirements,
        inputs: e.inputs,
        is_active: e.is_active,
    };
}

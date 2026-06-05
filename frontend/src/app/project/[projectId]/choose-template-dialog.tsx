'use client'

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useEffect, useState } from "react";
import { AppTemplateModel } from "@/shared/model/app-template.model"
import CreateTemplateAppSetupDialog from "./create-template-app-setup-dialog"
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import { Search, Loader2 } from "lucide-react";
import { useT } from "@/i18n";
import { listTemplates } from "./actions";
import type { TemplateDto } from "@/server/adapter/backend-api.adapter";

// Map a backend TemplateDto into the AppTemplateModel shape the downstream
// setup dialog expects. The image is now top-level on the DTO; we inject it
// into appModel.containerImageSource AND prepend it to inputSettings so the
// user can still override at deploy time.
function toAppTemplate(dto: TemplateDto): AppTemplateModel {
    const imageInputEntry = {
        key: "containerImageSource",
        label: "Container Image",
        value: dto.image_ref,
        isEnvVar: false,
        randomGeneratedIfEmpty: false,
    };

    return {
        name: dto.name,
        iconName: dto.icon_url ?? undefined,
        templates: [{
            appModel: {
                ...dto.spec?.appModel,
                // Pre-fill with the rendered ref so the app deploys even if
                // the user doesn't open the inputs section.
                containerImageSource: dto.image_ref,
            },
            appDomains: dto.spec?.appDomains ?? [],
            appVolumes: dto.spec?.appVolumes ?? [],
            appFileMounts: dto.spec?.appFileMounts ?? [],
            appPorts: dto.spec?.appPorts ?? [],
            inputSettings: [imageInputEntry, ...(dto.inputs ?? [])],
        }],
    } as AppTemplateModel;
}

export default function ChooseTemplateDialog({
    projectId,
    templateType,
    onClose
}: {
    projectId: string;
    templateType: 'database' | 'template' | undefined;
    onClose: () => void;
}) {

    const t = useT();
    const [isOpen, setIsOpen] = useState<boolean>(false);
    const [chosenAppTemplate, setChosenAppTemplate] = useState<AppTemplateModel | undefined>(undefined);
    const [chosenDto, setChosenDto] = useState<TemplateDto | undefined>(undefined);
    const [allTemplates, setAllTemplates] = useState<TemplateDto[]>([]);
    const [loading, setLoading] = useState<boolean>(false);
    const [searchQuery, setSearchQuery] = useState<string>("");

    // Fetch templates on first open. Cached for the dialog's lifetime.
    useEffect(() => {
        if (!templateType) return;
        setIsOpen(true);
        setSearchQuery("");
        if (allTemplates.length > 0) return;
        setLoading(true);
        listTemplates()
            .then(rows => setAllTemplates(rows as TemplateDto[]))
            .catch(() => setAllTemplates([]))
            .finally(() => setLoading(false));
    }, [templateType]);

    const wantedCategory = templateType === 'database' ? 'database' : 'app';
    const filteredTemplates = allTemplates
        .filter(t => t.category === wantedCategory)
        .filter(t => t.name.toLowerCase().includes(searchQuery.toLowerCase()))
        .sort((a, b) => a.name.localeCompare(b.name));

    return (
        <>
            <CreateTemplateAppSetupDialog appTemplate={chosenAppTemplate} templateDto={chosenDto} projectId={projectId}
                dialogClosed={() => {
                    setChosenAppTemplate(undefined);
                    setChosenDto(undefined);
                    onClose();
                }} />
            <Dialog open={!!isOpen} onOpenChange={(isOpened) => {
                setIsOpen(isOpened);
                if (!isOpened) {
                    onClose();
                }
            }}>
                <DialogContent className="sm:max-w-[1000px]">
                    <DialogHeader>
                        <DialogTitle>{templateType === 'database' ? t('templates.createDatabaseFromTemplate') : t('templates.createAppFromTemplate')}</DialogTitle>
                        <DialogDescription>
                            {t('templates.chooseDescription')}
                        </DialogDescription>
                    </DialogHeader>
                    <div className="relative mb-4">
                        <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-gray-400" />
                        <Input
                            type="text"
                            placeholder={t('templates.searchPlaceholder')}
                            value={searchQuery}
                            onChange={(e) => setSearchQuery(e.target.value)}
                            className="pl-10"
                        />
                    </div>
                    <ScrollArea className="max-h-[60vh]">
                        {loading ? (
                            <div className="flex justify-center py-12 text-muted-foreground">
                                <Loader2 className="h-6 w-6 animate-spin" />
                            </div>
                        ) : (
                            <div className="grid grid-cols-1 md:grid-cols-4 gap-4 px-1">
                                {filteredTemplates.map((dto) => {
                                    const iconSrc = dto.icon_url ?? undefined;
                                    return (
                                        <div key={dto.id}
                                            className="grid grid-cols-1 gap-2 items-center bg-white rounded-md p-4 border border-gray-200 text-center hover:bg-slate-50 active:bg-slate-100 transition-all cursor-pointer"
                                            onClick={() => {
                                                setIsOpen(false);
                                                setChosenDto(dto);
                                                setChosenAppTemplate(toAppTemplate(dto));
                                            }} >
                                            {iconSrc && <img src={iconSrc} className="h-10 mx-auto" />}
                                            <h3 className="text-lg font-semibold leading-tight">{dto.name}</h3>
                                            <div className="text-xs text-muted-foreground font-mono truncate" title={dto.image_ref}>
                                                {dto.image_repository}:{dto.image_tag}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        )}
                    </ScrollArea>
                </DialogContent>
            </Dialog>
        </>
    )
}

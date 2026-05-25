'use client'

export default function PageTitle({ title, subtitle, children }: {
    title: string;
    subtitle?: string;
    children?: React.ReactNode;
}) {
    return <div className="flex gap-4">
        <div className="flex-1 space-y-2">
            <h2 className="text-3xl font-bold tracking-tight ">{title}</h2>
            <p className="text-xs text-slate-600">{subtitle}</p>
        </div>
        {children}
    </div>
}
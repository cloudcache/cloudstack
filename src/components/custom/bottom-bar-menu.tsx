export default function BottomBarMenu({ children }: { children: React.ReactNode }) {
    return (<>
        <div className="flex w-full flex-col items-center left-0 bottom-0 fixed bg-white border-t z-50">
            <div className="w-full max-w-8xl px-4 lg:px-20">
                <div className="flex p-4 gap-4 items-center">
                    {children}
                </div>
            </div>
        </div>
        <div className="h-20"></div>
    </>
    )
}
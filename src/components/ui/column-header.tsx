import { cn } from "@/frontend/utils/utils"
import { Button } from "./button"
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuLabel } from "./dropdown-menu"
import {
    ArrowDownIcon,
    ArrowUpIcon,
    CaretSortIcon,
    EyeNoneIcon,
  } from "@radix-ui/react-icons"
  import { Column } from "@tanstack/react-table"
import { useState } from "react"
import { FilterIcon, FilterX, Trash2 } from "lucide-react"

  interface DataTableColumnHeaderProps<TData, TValue>
  extends React.HTMLAttributes<HTMLDivElement> {
  column: Column<TData, TValue>
  title: string
  filterOptions?: {
      accessorKey: string,
      filterLabel: string
  }[],
  disableSorting?: boolean
}

export function DataTableColumnHeader<TData, TValue>({
  column,
  title,
  className,
  filterOptions = [],
  disableSorting = false,
}: DataTableColumnHeaderProps<TData, TValue>) {
  const [tempFilters, setTempFilters] = useState<string[]>([])

  const handleFilterToggle = (option: string, column: Column<TData, TValue>) => {
      let newFilters: string[] | undefined = tempFilters.includes(option)
          ? tempFilters.filter(x => x !== option)
          : [...tempFilters, option]

      if (newFilters.length === 0) {
          newFilters = undefined;
      }
      setTempFilters(newFilters ?? []);
      column.setFilterValue(newFilters);
  }

  const clearFilters = () => {
      setTempFilters([]);
      column.setFilterValue(undefined);
  }

  if (!column.getCanSort() && filterOptions.length === 0) {
      return <div className={cn(className)}>{title}</div>
  }

  return (
      <div className={cn("flex items-center space-x-0.5", className)}>
          <DropdownMenu>
              <DropdownMenuTrigger asChild>
                  <Button
                      variant="ghost"
                      size="sm"
                      className="-ml-3 h-8 data-[state=open]:bg-accent"
                  >
                      <span>{title}</span>
                      {!disableSorting && <>
                          {
                              column.getIsSorted() === "desc" ? (
                                  <ArrowDownIcon className="ml-2 h-4 w-4" />
                              ) : column.getIsSorted() === "asc" ? (
                                  <ArrowUpIcon className="ml-2 h-4 w-4" />
                              ) : (
                                  <CaretSortIcon className="ml-2 h-4 w-4" />
                              )
                          }
                      </>}
                  </Button>
              </DropdownMenuTrigger>
              {!disableSorting &&<DropdownMenuContent align="start">
                  {/* Sorting options */}
                  <DropdownMenuItem onClick={() => column.toggleSorting(false)}>
                      <ArrowUpIcon className="mr-2 h-3.5 w-3.5 text-muted-foreground/70" />
                      Asc
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => column.toggleSorting(true)}>
                      <ArrowDownIcon className="mr-2 h-3.5 w-3.5 text-muted-foreground/70" />
                      Desc
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={() => column.toggleVisibility(false)}>
                      <EyeNoneIcon className="mr-2 h-3.5 w-3.5 text-muted-foreground/70" />
                      Hide
                  </DropdownMenuItem>
              </DropdownMenuContent>}
          </DropdownMenu>

          {filterOptions.length > 0 && (<DropdownMenu>
              <DropdownMenuTrigger asChild>
                  <Button
                      variant="ghost"
                      size="sm"
                      className="-ml-3 h-8 data-[state=open]:bg-accent"
                  >
                     {tempFilters.length === 0 ? <FilterIcon /> : <FilterX />}
                  </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                  {/* Filtering options */}
                  {filterOptions.length > 0 && (
                      <>
                          <DropdownMenuLabel>Filter Options</DropdownMenuLabel>
                          {filterOptions.map((option) => (
                              <DropdownMenuItem
                                  key={`${option.accessorKey}_${option.filterLabel}`}
                                  onClick={() => handleFilterToggle(`${option.accessorKey}_${option.filterLabel}`, column)}
                              >
                                  <input
                                      type="checkbox"
                                      readOnly
                                      checked={tempFilters.includes(`${option.accessorKey}_${option.filterLabel}`)}
                                      className="mr-2"
                                  />
                                  {option.filterLabel}
                              </DropdownMenuItem>
                          ))}
                          <DropdownMenuSeparator />
                          <DropdownMenuItem onClick={clearFilters}>
                              <Trash2 className="mr-1 h-3.5 w-3.5 text-muted-foreground/70" /> Clear Filters
                          </DropdownMenuItem>
                      </>
                  )}
              </DropdownMenuContent>
          </DropdownMenu>)}
      </div>
  )
}
import { useState } from 'react';
import { useNavigate, useLocation } from 'react-router';
import { Sidebar } from '../layout/Sidebar';
import type { SidebarItem } from '../../lib/types';

interface FinanceLayoutProps {
  children: React.ReactNode;
  onRefresh?: () => void;
}

const subNav = [
  { label: 'Dashboard', path: '/finance' },
  { label: 'Accounts', path: '/finance/accounts' },
  { label: 'Transactions', path: '/finance/transactions' },
  { label: 'Budgets', path: '/finance/budgets' },
  { label: 'Investments', path: '/finance/investments' },
  { label: 'Goals', path: '/finance/goals' },
  { label: 'Liabilities', path: '/finance/liabilities' },
];

export function FinanceLayout({ children }: FinanceLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Finance');

  const currentPath = location.pathname;

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Tasks') navigate('/');
          if (item === 'Chat') navigate('/chat');
        }}
      />

      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Full-width tab bar (matches tasks page pattern) */}
        <div className="h-14 bg-background flex items-center px-2 flex-shrink-0">
          <div className="flex-1 flex items-center gap-2">
            {subNav.map((item) => {
              const isActive = currentPath === item.path;
              return (
                <button
                  key={item.path}
                  onClick={() => navigate(item.path)}
                  className={`flex-1 py-2 rounded-md text-[13px] font-light transition-colors ${
                    isActive
                      ? 'bg-surface-highest text-white'
                      : 'bg-surface-low text-muted hover:bg-surface-base hover:text-secondary'
                  }`}
                >
                  {item.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {children}
        </div>
      </div>
    </div>
  );
}

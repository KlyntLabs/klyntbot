import { createBrowserRouter, Navigate } from 'react-router';
import Layout from './components/Layout';
import Chat from './pages/Chat';
import Tasks from './pages/Tasks';
import TaskDetail from './pages/TaskDetail';
import Projects from './pages/Projects';
import ProjectDetail from './pages/ProjectDetail';
import Plans from './pages/Plans';
import PlanDetail from './pages/PlanDetail';
import Calendar from './pages/Calendar';
import Cron from './pages/Cron';
import Skills from './pages/Skills';
import Finance from './pages/Finance';
import Settings from './pages/Settings';
import Setup from './pages/Setup';

/**
 * Route configuration for createBrowserRouter / createMemoryRouter.
 *
 * AC-16.3: All 10 routes resolve.
 * AC-16.5: /setup is outside the Layout shell (no nav rail).
 * Unknown routes redirect to / (Chat).
 */
export const routes = [
  {
    // Setup is outside Layout — full-screen wizard
    path: '/setup',
    element: <Setup />,
  },
  {
    // All other routes are nested under Layout
    path: '/',
    element: <Layout />,
    children: [
      { index: true, element: <Chat /> },
      { path: 'tasks', element: <Tasks /> },
      { path: 'tasks/:id', element: <TaskDetail /> },
      { path: 'projects', element: <Projects /> },
      { path: 'projects/:id', element: <ProjectDetail /> },
      { path: 'plans', element: <Plans /> },
      { path: 'plans/:id', element: <PlanDetail /> },
      { path: 'calendar', element: <Calendar /> },
      { path: 'cron', element: <Cron /> },
      { path: 'skills', element: <Skills /> },
      { path: 'finance', element: <Finance /> },
      { path: 'settings', element: <Settings /> },
      // Unknown routes redirect to Chat
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
];

export const router = createBrowserRouter(routes);

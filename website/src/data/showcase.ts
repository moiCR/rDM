import dashboard from '../assets/screenshots/dashboard.png';
import downloads from '../assets/screenshots/downloads.png';
import dashboardSpanish from '../assets/screenshots/dashboard-es.png';

export const showcaseSlides = [
  {
    image: dashboard,
    label: 'Overview',
    title: 'Everything at a glance.',
    description: 'Engine status, speed and download history. One view.',
    alt: 'Rust Download Manager dashboard showing engine status, speed, active tasks, transfers and download history.',
  },
  {
    image: downloads,
    label: 'Downloads',
    title: 'Room for your next download.',
    description: 'A clean workspace. Add a URL to get started.',
    alt: 'Rust Download Manager downloads view with an add button and the empty download list.',
  },
  {
    image: dashboardSpanish,
    label: 'Your language',
    title: 'Right at home.',
    description: 'The same overview, available in Spanish.',
    alt: 'Rust Download Manager dashboard in Spanish, with engine status, active tasks and download history.',
  },
] as const;

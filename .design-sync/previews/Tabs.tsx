import { Tabs } from '@unfour/ui';

const fileTabs = [
  { id: 'query', title: 'query.sql' },
  { id: 'schema', title: 'schema.sql', modified: true },
  { id: 'results', title: 'Results', loading: true },
];

export const Default = () => (
  <div style={{ width: 480 }}>
    <Tabs tabs={fileTabs} activeId="query" onSelect={() => {}} />
  </div>
);
export const WithClose = () => (
  <div style={{ width: 480 }}>
    <Tabs tabs={fileTabs} activeId="schema" onSelect={() => {}} onClose={() => {}} />
  </div>
);

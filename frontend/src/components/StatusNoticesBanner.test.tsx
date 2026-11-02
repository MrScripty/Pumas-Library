import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { StatusNoticesBanner } from './StatusNoticesBanner';

describe('StatusNoticesBanner', () => {
  it('renders nothing when the backend reports no degraded subsystem', () => {
    const { container } = render(<StatusNoticesBanner notices={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('shows each backend notice as a status message', () => {
    render(<StatusNoticesBanner notices={['Downloads are disabled because saved download state could not be read.']} />);
    expect(screen.getByRole('status')).toHaveTextContent(
      'Downloads are disabled because saved download state could not be read.'
    );
  });
});

for (const carousel of document.querySelectorAll<HTMLElement>('[data-carousel]')) {
  const viewport = carousel.querySelector<HTMLElement>('[data-carousel-viewport]');
  const slides = Array.from(carousel.querySelectorAll<HTMLElement>('[data-carousel-slide]'));
  const dots = Array.from(carousel.querySelectorAll<HTMLButtonElement>('[data-carousel-dot]'));
  const status = carousel.querySelector<HTMLElement>('[data-carousel-status]');
  const controls = carousel.querySelector<HTMLElement>('[data-carousel-controls]');
  if (!viewport || slides.length === 0) continue;
  let current = 0;

  function updateSelection(index: number) {
    if (current === index) return;
    current = index;
    for (const [dotIndex, dot] of dots.entries()) {
      dot.setAttribute('aria-pressed', String(dotIndex === current));
    }
    if (status) status.textContent = `Screenshot ${current + 1} of ${slides.length}: ${slides[current]?.getAttribute('aria-label')?.split(': ').slice(1).join(': ') ?? ''}`;
  }

  function navigate(index: number) {
    const next = (index + slides.length) % slides.length;
    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    viewport?.scrollTo({ left: next * viewport.clientWidth, behavior: reducedMotion ? 'instant' : 'smooth' });
  }

  for (const [index, dot] of dots.entries()) {
    dot.addEventListener('click', () => navigate(index));
  }
  carousel.querySelector('[data-carousel-previous]')?.addEventListener('click', () => navigate(current - 1));
  carousel.querySelector('[data-carousel-next]')?.addEventListener('click', () => navigate(current + 1));
  viewport.addEventListener('scroll', () => {
    if (viewport.clientWidth === 0) return;
    updateSelection(Math.max(0, Math.min(slides.length - 1, Math.round(viewport.scrollLeft / viewport.clientWidth))));
  }, { passive: true });
  viewport.addEventListener('keydown', (event) => {
    const destinations: Record<string, number> = { ArrowLeft: current - 1, ArrowRight: current + 1, Home: 0, End: slides.length - 1 };
    const destination = destinations[event.key];
    if (destination === undefined) return;
    event.preventDefault();
    navigate(destination);
  });
  if (controls) controls.hidden = false;
}

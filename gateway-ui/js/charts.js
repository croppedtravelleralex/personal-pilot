let chartLoaderPromise = null;

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export async function ensureChartJs() {
  if (window.Chart) return window.Chart;
  if (chartLoaderPromise) return chartLoaderPromise;
  chartLoaderPromise = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = 'https://cdn.jsdelivr.net/npm/chart.js@4.4.3/dist/chart.umd.min.js';
    script.async = true;
    script.onload = () => resolve(window.Chart);
    script.onerror = () => reject(new Error('Chart.js CDN 加载失败'));
    document.head.appendChild(script);
    window.setTimeout(() => {
      if (!window.Chart) reject(new Error('Chart.js CDN 加载超时'));
    }, 4000);
  });
  return chartLoaderPromise;
}

export async function renderTaskDonut(container, counts) {
  if (!container) return;
  const total = Number(counts?.total || 0);
  const succeeded = Number(counts?.succeeded || 0);
  const failed = Number(counts?.failed || 0);
  const timedOut = Number(counts?.timed_out || 0);
  const cancelled = Number(counts?.cancelled || 0);
  const segments = [succeeded, failed, timedOut, cancelled];
  const colors = [cssVar('--ok') || '#5be49b', cssVar('--danger') || '#ff7a8d', cssVar('--warn') || '#ffcc66', cssVar('--info') || '#79c9ff'];

  container.innerHTML = '<div class="donut-fallback"><div class="donut-center">任务<br />分布</div></div>';
  const fallback = container.querySelector('.donut-fallback');
  const percentages = total > 0 ? segments.map((value) => (value / total) * 100) : [25, 25, 25, 25];
  fallback.style.background = `conic-gradient(${colors[0]} 0 ${percentages[0]}%, ${colors[1]} ${percentages[0]}% ${percentages[0] + percentages[1]}%, ${colors[2]} ${percentages[0] + percentages[1]}% ${percentages[0] + percentages[1] + percentages[2]}%, ${colors[3]} ${percentages[0] + percentages[1] + percentages[2]}% 100%)`;

  try {
    const Chart = await ensureChartJs();
    container.innerHTML = '<canvas width="120" height="120" aria-label="任务成功率环形图"></canvas>';
    const canvas = container.querySelector('canvas');
    if (canvas.__chartInstance) {
      canvas.__chartInstance.destroy();
    }
    const chart = new Chart(canvas, {
      type: 'doughnut',
      data: {
        labels: ['成功', '失败', '超时', '取消'],
        datasets: [{ data: segments, backgroundColor: colors, borderWidth: 0 }],
      },
      options: {
        animation: { duration: 320 },
        plugins: { legend: { display: false }, tooltip: { enabled: true } },
        cutout: '66%',
      },
    });
    canvas.__chartInstance = chart;
  } catch {
    // 保持 CSS 环图降级结果即可。
  }
}

if [ ! -f "vk_tracer/render_graph_nodes.dot" ]; then
  echo "Can't find 'vk_tracer/render_graph_nodes.dot' !"
  exit 1
fi
if [ ! -f "vk_tracer/render_graph_timeline.mmd" ]; then
  echo "Can't find 'vk_tracer/render_graph_timeline.mmd' !"
  exit 1
fi

echo "Rendering nodes..."
dot -Tpng -Gsize=20,20\! -Gdpi=100 -o vk_tracer/render_graph_nodes.png vk_tracer/render_graph_nodes.dot
if [ "$?" -ne 0 ]; then
  echo "Failed to render nodes !"
  exit 1
fi

echo "Rendering timeline..."
mmdc -i vk_tracer/render_graph_timeline.mmd -o vk_tracer/render_graph_timeline.png -w 2000
if [ "$?" -ne 0 ]; then
  echo "Failed to render timeline !"
  exit 1
fi

echo "Stitching..."
convert -append vk_tracer/render_graph_nodes.png vk_tracer/render_graph_timeline.png vk_tracer/render_graph_final.png
if [ "$?" -ne 0 ]; then
  echo "Failed to stitch renders !"
  exit 1
fi

echo "Done !"

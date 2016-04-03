---
layout: default
title: Blog
permalink: /blog/
---
  <ul class="post-list">
    {% for post in site.posts %}
      <li>
        <span class="post-meta">{{ post.date | date: "%b %-d, %Y" }}</span>

        <h2>
          <a class="post-link" href="{{ post.url | prepend: site.baseurl }}">{{ post.title }}</a>
        </h2>
        {% if post.content.size > 300 %}
           {{ post.content | truncatewords: 150 }}
           <a href="{{ post.url }}">read more</a>
        {% else %}
           {{ post.content }}
        {% endif %}
      </li>
    {% endfor %}
  </ul>

  <br/>
  <p class="rss-subscribe">subscribe <a href="{{ "/feed.xml" | prepend: site.baseurl }}">via RSS</a></p>
